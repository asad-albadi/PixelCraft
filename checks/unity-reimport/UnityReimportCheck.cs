using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Animations;
using PackageInfo = UnityEditor.PackageManager.PackageInfo;
using UnityEngine;

namespace PixelCraft.Checks
{
    public static class UnityReimportCheck
    {
        private const string SheetPath = "Assets/hero.png";
        private const string JsonPath = "Assets/hero.json";
        private const string ClipFolder = "Assets/hero.PixelCraftClips";
        private const string ControllerPath = "Assets/Hero.controller";
        private const string DirectPath = "Assets/Direct/hero.aseprite";
        private const string DirectControllerPath = "Assets/Direct/Hero.controller";
        private const string SheetBaselinePath = "Fixture/unity-sheet-first.json";
        private const string DirectBaselinePath = "Fixture/unity-aseprite-first.json";
        private static readonly string[] Tags = { "idle", "walk", "jump" };

        [Serializable]
        private sealed class Sheet
        {
            public Frame[] frames;
            public Metadata meta;
        }

        [Serializable]
        private sealed class Frame
        {
            public string filename;
            public int frameId;
            public int sourceFrame;
            public Box frame;
            public Box sourceSize;
            public Box spriteSourceSize;
            public int duration;
            public bool trimmed;
            public bool rotated;
        }

        [Serializable]
        private sealed class Box
        {
            public int x;
            public int y;
            public int w;
            public int h;
        }

        [Serializable]
        private sealed class Metadata
        {
            public string app;
            public string version;
            public string image;
            public Tag[] frameTags;
            public Box size;
        }

        [Serializable]
        private sealed class Tag
        {
            public string name;
            public int from;
            public int to;
            public int repeat;
            public string direction;
        }

        [Serializable]
        private sealed class Identity
        {
            public string name;
            public string guid;
            public long id;
        }

        [Serializable]
        private sealed class Baseline
        {
            public Identity[] sprites;
            public Identity[] clips;
        }

        public static void Run()
        {
            string phase = Argument("-pixelcraftPhase");
            string route = Argument("-pixelcraftRoute");
            Require(phase == "first" || phase == "redraw" || phase == "insert", "Invalid phase");
            var options = ImportAssetOptions.ForceSynchronousImport | ImportAssetOptions.ForceUpdate;
            if (route == "sheet")
                CheckSheet(phase, options);
            else if (route == "aseprite")
                CheckDirect(phase, options);
            else
                throw new InvalidOperationException("Invalid route " + route);
        }

        private static string Argument(string name)
        {
            var args = Environment.GetCommandLineArgs();
            int index = Array.IndexOf(args, name);
            Require(index >= 0 && index + 1 < args.Length, "Missing " + name);
            return args[index + 1];
        }

        private static void CheckSheet(string phase, ImportAssetOptions options)
        {
            bool first = phase == "first";
            bool inserted = phase == "insert";
            AssetDatabase.ImportAsset(SheetPath, options);
            AssetDatabase.ImportAsset(JsonPath, options);
            PixelCraft.Editor.SpriteSheetImporter.Import(JsonPath);
            AssetDatabase.SaveAssets();

            var sheet = JsonUtility.FromJson<Sheet>(File.ReadAllText(JsonPath));
            ValidateSheet(sheet, inserted);
            var sprites = SubAssets<Sprite>(SheetPath);
            Require(sprites.Length == sheet.frames.Length, "Sprite count does not match JSON");
            var byName = sprites.ToDictionary(sprite => sprite.name);
            var importer = AssetImporter.GetAtPath(SheetPath) as TextureImporter;
            Require(importer != null && importer.spriteImportMode == SpriteImportMode.Multiple &&
                    importer.filterMode == FilterMode.Point, "Expected point-filtered multiframe texture");
            foreach (var info in sheet.frames)
            {
                Require(byName.TryGetValue(info.filename, out var sprite), "Missing Unity sprite " + info.filename);
                var box = info.frame;
                var rect = new Rect(box.x, sheet.meta.size.h - box.y - box.h, box.w, box.h);
                Require(sprite.rect == rect, "Unity sprite rectangle mismatch for " + sprite.name);
            }

            var clips = AssetDatabase.FindAssets("t:AnimationClip", new[] { ClipFolder })
                .Select(guid => AssetDatabase.LoadAssetAtPath<AnimationClip>(AssetDatabase.GUIDToAssetPath(guid)))
                .ToDictionary(clip => clip.name);
            Require(clips.Count == Tags.Length, "Expected three generated AnimationClip assets");
            var controller = first ? AnimatorController.CreateAnimatorControllerAtPath(ControllerPath)
                                   : AssetDatabase.LoadAssetAtPath<AnimatorController>(ControllerPath);
            Require(controller != null, "Animator Controller could not be loaded");
            var stateMachine = controller.layers[0].stateMachine;
            foreach (string tag in Tags)
            {
                Require(clips.TryGetValue(tag, out var clip), "Missing clip " + tag);
                if (first)
                {
                    var newState = stateMachine.AddState(tag);
                    newState.motion = clip;
                    EditorUtility.SetDirty(newState);
                }
                var state = stateMachine.states.Single(entry => entry.state.name == tag).state;
                Require(state.motion == clip, "Controller lost the original clip reference for " + tag);
                ValidateClip(clip, sheet, byName, tag);
            }
            EditorUtility.SetDirty(controller);
            AssetDatabase.SaveAssets();

            var spriteIds = sprites.Select(Identify).ToArray();
            var clipIds = clips.Values.Select(Identify).ToArray();
            if (first)
            {
                var baseline = new Baseline { sprites = spriteIds, clips = clipIds };
                File.WriteAllText(SheetBaselinePath, JsonUtility.ToJson(baseline, true));
            }
            else
            {
                var baseline = JsonUtility.FromJson<Baseline>(File.ReadAllText(SheetBaselinePath));
                ValidatePreserved("Sprite", baseline.sprites, spriteIds);
                ValidatePreserved("AnimationClip", baseline.clips, clipIds);
            }
            ValidateRedrawFromController(controller, sheet, first, inserted);
            if (first)
                CheckMismatchedRectangle(sheet, clips, byName);
            Debug.Log($"PC_SHEET {phase} passed");
        }

        private static void CheckMismatchedRectangle(
            Sheet sheet, Dictionary<string, AnimationClip> clips, Dictionary<string, Sprite> sprites)
        {
            var original = File.ReadAllText(JsonPath);
            sheet.frames[0].frame.x++;
            File.WriteAllText(JsonPath, JsonUtility.ToJson(sheet));
            try
            {
                PixelCraft.Editor.SpriteSheetImporter.Import(JsonPath);
                throw new InvalidOperationException("Unity accepted a same-named sprite with the wrong rectangle");
            }
            catch (InvalidDataException error)
            {
                Require(error.Message.Contains("does not match the sheet rectangle"), "Unexpected rejection: " + error.Message);
            }
            finally
            {
                File.WriteAllText(JsonPath, original);
                sheet.frames[0].frame.x--;
            }
            foreach (var entry in clips)
                ValidateClip(entry.Value, sheet, sprites, entry.Key);
            Debug.Log("PC_SHEET mismatched sprite rectangle rejected");
        }

        private static void CheckDirect(string phase, ImportAssetOptions options)
        {
            bool first = phase == "first";
            int expectedFrames = phase == "insert" ? 7 : 6;
            AssetDatabase.ImportAsset(DirectPath, options);
            var importer = AssetImporter.GetAtPath(DirectPath);
            var package = importer == null ? null : PackageInfo.FindForAssembly(importer.GetType().Assembly);
            Require(package?.name == "com.unity.2d.aseprite",
                ".aseprite asset was not handled by the official importer");
            var directSprites = SubAssets<Sprite>(DirectPath);
            var directClips = SubAssets<AnimationClip>(DirectPath);
            bool hasTags = Tags.All(tag => directClips.Any(clip => clip.name == tag));
            Require(directSprites.Length >= expectedFrames, "Official Aseprite import lost frame sprites");
            Require(hasTags, "Official Aseprite import lost tags. Clips: " +
                string.Join(",", directClips.Select(clip => clip.name)));
            var controller = first
                ? AnimatorController.CreateAnimatorControllerAtPath(DirectControllerPath)
                : AssetDatabase.LoadAssetAtPath<AnimatorController>(DirectControllerPath);
            Require(controller != null, "Direct ASE Animator Controller could not be loaded");
            var stateMachine = controller.layers[0].stateMachine;
            var resolved = 0;
            foreach (var tag in Tags)
            {
                var clip = directClips.Single(item => item.name == tag);
                if (first)
                {
                    var state = stateMachine.AddState(tag);
                    state.motion = clip;
                    EditorUtility.SetDirty(state);
                }
                var saved = stateMachine.states.Single(item => item.state.name == tag).state;
                if (saved.motion == clip)
                    resolved++;
            }
            if (first)
            {
                Require(resolved == Tags.Length, "Initial direct ASE controller clips could not be assigned");
                EditorUtility.SetDirty(controller);
                AssetDatabase.SaveAssets();
            }
            Debug.Log($"PC_ASE {phase} existing Animator Controller references resolved {resolved}/{Tags.Length}");

            var directSpriteIds = directSprites.Select(Identify).ToArray();
            var directClipIds = directClips.Select(Identify).ToArray();
            string versions = $"Unity {Application.unityVersion} Aseprite {package.version}";
            string details = "";
            if (!first)
            {
                var baseline = JsonUtility.FromJson<Baseline>(File.ReadAllText(DirectBaselinePath));
                details = $", unchanged named sprite identities {Matching(baseline.sprites, directSpriteIds)}/{baseline.sprites.Length}" +
                    $", unchanged clip identities {Matching(baseline.clips, directClipIds)}/{baseline.clips.Length}";

                // The final walk key represents the same authored cel after the
                // insertion. Matching by Frame_N alone can hide an index-based ID.
                var priorWalkTail = baseline.sprites.Single(sprite => sprite.name == "Frame_3");
                var walk = directClips.Single(clip => clip.name == "walk");
                var binding = AnimationUtility.GetObjectReferenceCurveBindings(walk)
                    .Single(candidate => candidate.type == typeof(SpriteRenderer));
                var currentWalkTail = Identify(AnimationUtility.GetObjectReferenceCurve(walk, binding).Last().value);
                bool kept = priorWalkTail.guid == currentWalkTail.guid && priorWalkTail.id == currentWalkTail.id;
                Debug.Log($"PC_ASE {phase} surviving walk cel identity preserved={kept} ({priorWalkTail.name} → {currentWalkTail.name})");
            }
            Debug.Log($"PC_ASE {phase} imported {directSprites.Length} sprites and {directClips.Length} clips ({versions}){details}");
            foreach (var clip in directClips.OrderBy(clip => clip.name))
            {
                var binding = AnimationUtility.GetObjectReferenceCurveBindings(clip)
                    .FirstOrDefault(candidate => candidate.type == typeof(SpriteRenderer));
                string keys = binding.type == null ? "no SpriteRenderer object curve" :
                    string.Join(",", AnimationUtility.GetObjectReferenceCurve(clip, binding)
                        .Select(key => $"{key.time * 1000:0.###}ms={key.value?.name ?? "null"}"));
                Debug.Log($"PC_ASE {phase} {clip.name} length={clip.length * 1000:0.###}ms: {keys}");
            }
            if (first)
            {
                var baseline = new Baseline { sprites = directSpriteIds, clips = directClipIds };
                File.WriteAllText(DirectBaselinePath, JsonUtility.ToJson(baseline, true));
            }
            Debug.Log($"PC_ASE {phase} passed");
        }

        private static T[] SubAssets<T>(string path) where T : UnityEngine.Object =>
            AssetDatabase.LoadAllAssetsAtPath(path).OfType<T>().ToArray();

        private static void ValidateSheet(Sheet sheet, bool inserted)
        {
            var ids = inserted ? new[] { 0, 1, 2, 6, 3, 4, 5 } : new[] { 0, 1, 2, 3, 4, 5 };
            var durations = inserted ? new[] { 93, 147, 71, 111, 173, 121, 203 } :
                new[] { 93, 147, 71, 129, 121, 203 };
            Require(sheet.frames.Length == ids.Length, "Unexpected JSON frame count");
            Require(sheet.meta.size.w == 32 && sheet.meta.size.h == 12, "Unexpected atlas dimensions");
            for (int i = 0; i < ids.Length; ++i)
            {
                var frame = sheet.frames[i];
                Require(frame.filename == $"frame_{ids[i]}" && frame.frameId == ids[i] &&
                    frame.sourceFrame == i && frame.duration == durations[i], $"Frame identity, order or timing mismatch at {i}");
                Require(!frame.trimmed && !frame.rotated && frame.frame.x == i % 4 * 8 &&
                    frame.frame.y == i / 4 * 6 && frame.frame.w == 8 && frame.frame.h == 6 &&
                    frame.sourceSize.w == 8 && frame.sourceSize.h == 6 &&
                    frame.spriteSourceSize.x == 0 && frame.spriteSourceSize.y == 0 &&
                    frame.spriteSourceSize.w == 8 && frame.spriteSourceSize.h == 6,
                    $"Expected fixed-size untrimmed cell at {i}");
            }

            var starts = inserted ? new[] { 0, 2, 5 } : new[] { 0, 2, 4 };
            var ends = inserted ? new[] { 1, 4, 6 } : new[] { 1, 3, 5 };
            Require(sheet.meta.frameTags.Length == Tags.Length, "Unexpected tag count");
            for (int i = 0; i < Tags.Length; ++i)
            {
                var tag = sheet.meta.frameTags[i];
                Require(tag.name == Tags[i] && tag.from == starts[i] && tag.to == ends[i] &&
                    tag.direction == "forward" && tag.repeat == 0, "Unexpected range for tag " + Tags[i]);
            }
        }

        private static void ValidateClip(AnimationClip clip, Sheet sheet, Dictionary<string, Sprite> sprites, string name)
        {
            var tag = sheet.meta.frameTags.Single(value => value.name == name);
            var binding = AnimationUtility.GetObjectReferenceCurveBindings(clip)
                .Single(value => value.type == typeof(SpriteRenderer) && value.propertyName == "m_Sprite");
            var keys = AnimationUtility.GetObjectReferenceCurve(clip, binding);
            Require(keys.Length == tag.to - tag.from + 2, "Wrong number of sprite keys for " + name);
            float time = 0;
            for (int frame = tag.from; frame <= tag.to; ++frame)
            {
                var key = keys[frame - tag.from];
                Require(Mathf.Abs(key.time - time) < 0.0001f &&
                    key.value == sprites[sheet.frames[frame].filename], "Wrong sprite or time in " + name);
                time += sheet.frames[frame].duration / 1000f;
            }
            Require(Mathf.Abs(keys.Last().time - (time - 1f / clip.frameRate)) < 0.0001f &&
                keys.Last().value == sprites[sheet.frames[tag.to].filename] &&
                Mathf.Abs(clip.length - time) < 0.0001f,
                "Incorrect terminal sprite or clip duration for " + name);
            Require(clip.frameRate == 1000 && AnimationUtility.GetAnimationClipSettings(clip).loopTime,
                "Expected millisecond resolution and looping for " + name);
        }

        private static void ValidateRedrawFromController(AnimatorController controller, Sheet sheet, bool first, bool inserted)
        {
            var walk = (AnimationClip)controller.layers[0].stateMachine.states
                .Single(entry => entry.state.name == "walk").state.motion;
            var actor = new GameObject("Hero check");
            var renderer = actor.AddComponent<SpriteRenderer>();
            actor.AddComponent<Animator>().runtimeAnimatorController = controller;
            var texture = new Texture2D(2, 2, TextureFormat.RGBA32, false);
            AnimationMode.StartAnimationMode();
            try
            {
                AnimationMode.BeginSampling();
                try
                {
                    AnimationMode.SampleAnimationClip(actor, walk, inserted ? 0.19f : 0.08f);
                }
                finally
                {
                    AnimationMode.EndSampling();
                }
                Require(renderer.sprite != null && renderer.sprite.name == "frame_3",
                    "Sampling the controller's walk clip did not resolve the original third sprite");
                Require(ImageConversion.LoadImage(texture, File.ReadAllBytes(SheetPath)), "Could not decode sheet PNG");
                var rect = renderer.sprite.rect;
                Require(texture.width == sheet.meta.size.w && texture.height == sheet.meta.size.h, "PNG dimensions differ from JSON");
                var pixels = texture.GetPixels32();
                var changed = pixels[((int)rect.y + 4) * texture.width + (int)rect.x + 1];
                var center = pixels[((int)rect.y + 3) * texture.width + (int)rect.x + 2];
                Require(changed.a == (first ? 0 : 255) &&
                    center.Equals(first ? new Color32(25, 10, 235, 255) : new Color32(25, 180, 230, 255)),
                    "Sampled sprite did not resolve to the expected source pixels and redraw bounds");
            }
            finally
            {
                AnimationMode.StopAnimationMode();
                UnityEngine.Object.DestroyImmediate(actor);
                UnityEngine.Object.DestroyImmediate(texture);
            }
        }

        private static Identity Identify(UnityEngine.Object asset)
        {
            Require(AssetDatabase.TryGetGUIDAndLocalFileIdentifier(asset, out string guid, out long id),
                "Cannot obtain asset GUID and local file ID for " + asset.name);
            return new Identity { name = asset.name, guid = guid, id = id };
        }

        private static int Matching(Identity[] before, Identity[] after) =>
            before.Count(old => after.Any(current => current.name == old.name &&
                current.guid == old.guid && current.id == old.id));

        private static void ValidatePreserved(string kind, Identity[] before, Identity[] after)
        {
            foreach (var old in before)
            {
                Require(after.Any(current => current.name == old.name && current.guid == old.guid && current.id == old.id),
                    $"{kind} {old.name} lost its original GUID/local file ID ({old.guid}/{old.id})");
            }
        }

        private static void Require(bool condition, string error)
        {
            if (!condition)
                throw new InvalidOperationException(error);
        }
    }
}
