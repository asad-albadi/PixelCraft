using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using UnityEditor;
using UnityEngine;

namespace PixelCraft.Editor
{
    /// <summary>Creates or refreshes animation clips for a PixelCraft Unity sheet.</summary>
    public static class SpriteSheetImporter
    {
        private const string Menu = "Assets/PixelCraft/Import or Update Animation Clips";
        private const int KeyBudget = 1_000_000;
        private const long MaxClipMilliseconds = 8_000_000;
        private static readonly EditorCurveBinding SpriteCurve =
            EditorCurveBinding.PPtrCurve("", typeof(SpriteRenderer), "m_Sprite");

        [Serializable] private sealed class Sheet { public Frame[] frames; public Metadata meta; }
        [Serializable] private sealed class Frame { public string filename; public int duration; }
        [Serializable] private sealed class Metadata
        {
            public string app;
            public string version;
            public string image;
            public Tag[] frameTags;
        }
        [Serializable] private sealed class Tag
        {
            public string name;
            public int from;
            public int to;
            public string direction;
            public int repeat;
        }

        private sealed class ClipUpdate
        {
            public string Name;
            public string Path;
            public bool Loop;
            public AnimationClip Existing;
            public ObjectReferenceKeyframe[] Keys;
        }

        [MenuItem(Menu, true)]
        private static bool CanImportSelected()
        {
            var path = AssetDatabase.GetAssetPath(Selection.activeObject);
            return path.StartsWith("Assets/", StringComparison.Ordinal) &&
                   path.EndsWith(".json", StringComparison.OrdinalIgnoreCase);
        }

        [MenuItem(Menu)]
        private static void ImportSelected()
        {
            try
            {
                Import(AssetDatabase.GetAssetPath(Selection.activeObject));
            }
            catch (Exception error)
            {
                Debug.LogError($"PixelCraft could not import the sprite sheet: {error.Message}");
            }
        }

        /// <summary>Imports an Assets-relative PixelCraft sheet JSON, or throws with a diagnostic.</summary>
        public static void Import(string jsonAssetPath)
        {
            RequireSheetPath(jsonAssetPath);
            var input = new FileInfo(jsonAssetPath);
            if (!input.Exists || input.Length > 64L * 1024 * 1024)
                throw new InvalidDataException("The sheet JSON is missing or exceeds 64 MiB.");
            var sheet = JsonUtility.FromJson<Sheet>(File.ReadAllText(jsonAssetPath));
            if (sheet?.meta?.app != "PixelCraft" || sheet.meta.version != "1" ||
                sheet.frames == null || sheet.frames.Length == 0 || sheet.frames.Length > 65535)
                throw new InvalidDataException("Select a PixelCraft sprite-sheet JSON export.");

            var image = sheet.meta.image;
            if (string.IsNullOrEmpty(image) || image.IndexOfAny(new[] { '/', '\\', ':', '\0' }) >= 0 ||
                !image.EndsWith(".png", StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("The sheet must reference a PNG beside its JSON.");

            var parent = jsonAssetPath.Substring(0, jsonAssetPath.LastIndexOf('/'));
            var imagePath = $"{parent}/{image}";
            if (!File.Exists(imagePath))
                throw new FileNotFoundException("Copy the PNG and its Unity .meta file next to the JSON.", imagePath);

            var tags = sheet.meta.frameTags;
            if (tags == null || tags.Length == 0)
                tags = new[] { new Tag { name = "default", to = sheet.frames.Length - 1, direction = "forward" } };

            // Read the new texture before resolving the Sprite subassets. The matching
            // .meta file carries the existing texture GUID and local file IDs.
            AssetDatabase.ImportAsset(imagePath, ImportAssetOptions.ForceSynchronousImport | ImportAssetOptions.ForceUpdate);
            var textureGuid = AssetDatabase.AssetPathToGUID(imagePath);
            var sprites = AssetDatabase.LoadAllAssetsAtPath(imagePath).OfType<Sprite>()
                .ToDictionary(sprite => sprite.name, StringComparer.Ordinal);
            var spriteFrames = new Sprite[sheet.frames.Length];
            var frameNames = new HashSet<string>(StringComparer.Ordinal);
            for (var i = 0; i < sheet.frames.Length; i++)
            {
                var frame = sheet.frames[i];
                if (frame == null || string.IsNullOrEmpty(frame.filename) ||
                    frame.duration < 1 || frame.duration > 65535 || !frameNames.Add(frame.filename))
                    throw new InvalidDataException($"Invalid frame name or duration at index {i}.");
                if (!sprites.TryGetValue(frame.filename, out spriteFrames[i]))
                    throw new InvalidDataException(
                        $"Sprite '{frame.filename}' is missing in Unity. Copy the .png.meta from PixelCraft's Unity preset.");
            }

            var folder = $"{parent}/{Path.GetFileNameWithoutExtension(jsonAssetPath)}.PixelCraftClips";
            var ownership = "PixelCraftSheet_" + textureGuid;
            var tagNames = new HashSet<string>(StringComparer.Ordinal);
            var updates = new List<ClipUpdate>();
            var remainingKeys = KeyBudget;
            foreach (var tag in tags)
            {
                if (tag == null || string.IsNullOrWhiteSpace(tag.name) || !tagNames.Add(tag.name))
                    throw new InvalidDataException("Give every animation tag a unique, non-empty name.");
                var path = $"{folder}/{ClipFileName(tag.name)}.anim";
                var existing = AssetDatabase.LoadMainAssetAtPath(path);
                if ((existing != null && (!(existing is AnimationClip) || !AssetDatabase.GetLabels(existing).Contains(ownership))) ||
                    (existing == null && File.Exists(path)))
                    throw new IOException($"An unrelated asset occupies '{path}'. Move it before importing.");
                updates.Add(new ClipUpdate
                {
                    Name = tag.name,
                    Path = path,
                    Existing = existing as AnimationClip,
                    Loop = tag.repeat == 0,
                    Keys = CreateKeys(tag, sheet.frames, spriteFrames, ref remainingKeys),
                });
            }

            // Validate every destination and curve before modifying any clip.
            if (!AssetDatabase.IsValidFolder(folder) &&
                string.IsNullOrEmpty(AssetDatabase.CreateFolder(parent, Path.GetFileName(folder))))
                throw new IOException($"Could not create '{folder}'.");
            foreach (var update in updates)
            {
                var clip = update.Existing ?? new AnimationClip { name = update.Name };
                clip.frameRate = 1000;
                AnimationUtility.SetObjectReferenceCurve(clip, SpriteCurve, update.Keys);
                var settings = AnimationUtility.GetAnimationClipSettings(clip);
                settings.loopTime = update.Loop;
                AnimationUtility.SetAnimationClipSettings(clip, settings);
                if (update.Existing == null)
                {
                    AssetDatabase.CreateAsset(clip, update.Path);
                    AssetDatabase.SetLabels(clip, new[] { ownership });
                }
                clip.name = update.Name;
                EditorUtility.SetDirty(clip);
            }
            AssetDatabase.SaveAssets();
            Debug.Log($"PixelCraft imported {updates.Count} animation clip(s) into '{folder}'.");
        }

        private static ObjectReferenceKeyframe[] CreateKeys(
            Tag tag, Frame[] frames, Sprite[] sprites, ref int remainingKeys)
        {
            if (tag.from < 0 || tag.to < tag.from || tag.to >= frames.Length ||
                tag.repeat < 0 || tag.repeat > 65535)
                throw new InvalidDataException($"Invalid frame range or repeat count for tag '{tag.name}'.");
            var reverse = tag.direction == "reverse" || tag.direction == "pingpong_reverse";
            var pingPong = tag.direction == "pingpong" || tag.direction == "pingpong_reverse";
            if (!reverse && !pingPong && tag.direction != "forward")
                throw new InvalidDataException($"Unsupported animation direction for tag '{tag.name}'.");

            var span = tag.to - tag.from + 1;
            var step = reverse ? -1 : 1;
            var first = reverse ? tag.to : tag.from;
            var cycle = Enumerable.Range(0, span).Select(i => first + step * i).ToList();
            if (pingPong)
                for (var i = span - 2; i > 0; i--)
                    cycle.Add(cycle[i]);

            var repeats = Math.Max(tag.repeat, 1);
            var finalDuration = frames[cycle[cycle.Count - 1]].duration;
            var keyCount = (long)cycle.Count * repeats + (finalDuration > 1 ? 1 : 0);
            var duration = cycle.Sum(i => (long)frames[i].duration) * repeats;
            if (keyCount > remainingKeys || duration > MaxClipMilliseconds)
                throw new InvalidDataException($"Animation tag '{tag.name}' exceeds the clip time or key limit.");
            remainingKeys -= (int)keyCount;

            var keys = new ObjectReferenceKeyframe[(int)keyCount];
            var index = 0;
            long milliseconds = 0;
            for (var repeat = 0; repeat < repeats; repeat++)
            {
                foreach (var frame in cycle)
                {
                    keys[index++] = new ObjectReferenceKeyframe { time = milliseconds / 1000f, value = sprites[frame] };
                    milliseconds += frames[frame].duration;
                }
            }
            // Unity adds one sampling interval to the last object-reference key.
            if (finalDuration > 1)
                keys[index] = new ObjectReferenceKeyframe { time = (milliseconds - 1) / 1000f, value = keys[index - 1].value };
            return keys;
        }

        private static void RequireSheetPath(string path)
        {
            if (string.IsNullOrEmpty(path) || !path.StartsWith("Assets/", StringComparison.Ordinal) ||
                !path.EndsWith(".json", StringComparison.OrdinalIgnoreCase) ||
                path.IndexOfAny(new[] { '\\', ':', '\0' }) >= 0 ||
                path.Split('/').Any(part => part == "." || part == ".." || part.Length == 0))
                throw new ArgumentException("Choose a PNG sprite-sheet JSON inside the Unity project's Assets folder.");
        }

        private static string ClipFileName(string name)
        {
            // Filenames stay stable on case-insensitive file systems, including for
            // tag names that sanitize to the same readable prefix.
            var prefix = new string(name.Take(32).Select(c =>
                c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' ? c : '_').ToArray());
            using (var sha = SHA256.Create())
            {
                var hash = string.Concat(sha.ComputeHash(Encoding.UTF8.GetBytes(name))
                    .Select(value => value.ToString("x2", CultureInfo.InvariantCulture)));
                return $"{prefix}-{hash}";
            }
        }
    }
}
