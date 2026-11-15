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
        private const int ClipBudget = 1024;
        private const long MaxClipMilliseconds = 8_000_000;
        private static readonly EditorCurveBinding SpriteCurve =
            EditorCurveBinding.PPtrCurve("", typeof(SpriteRenderer), "m_Sprite");

        [Serializable] private sealed class Sheet { public Frame[] frames; public Metadata meta; }
        [Serializable] private sealed class Frame
        {
            public string filename;
            public int duration;
            public Box frame;
            public bool rotated;
        }
        [Serializable] private sealed class Box { public int x, y, w, h; }
        [Serializable] private sealed class Metadata
        {
            public string app;
            public string version;
            public string image;
            public Box size;
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
            public (float Time, int Frame)[] Keys;
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
            Sheet sheet;
            using (var input = File.Open(jsonAssetPath, FileMode.Open, FileAccess.Read, FileShare.Read))
            using (var reader = new StreamReader(input))
            {
                if (input.Length > 64L * 1024 * 1024)
                    throw new InvalidDataException("The sheet JSON exceeds 64 MiB.");
                sheet = JsonUtility.FromJson<Sheet>(reader.ReadToEnd());
            }
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
            if (tags.Length > ClipBudget)
                throw new InvalidDataException($"A sheet can import at most {ClipBudget} animation clips.");

            var size = sheet.meta.size;
            if (size == null || size.w <= 0 || size.h <= 0)
                throw new InvalidDataException("The sheet must declare a positive atlas size.");
            var frameNames = new HashSet<string>(StringComparer.Ordinal);
            for (var i = 0; i < sheet.frames.Length; i++)
            {
                var frame = sheet.frames[i];
                if (frame == null || string.IsNullOrEmpty(frame.filename) ||
                    frame.duration < 1 || frame.duration > 65535 || !frameNames.Add(frame.filename))
                    throw new InvalidDataException($"Invalid frame name or duration at index {i}.");
                var box = frame.frame;
                if (frame.rotated || box == null || box.x < 0 || box.y < 0 || box.w <= 0 || box.h <= 0 ||
                    box.x + (long)box.w > size.w || box.y + (long)box.h > size.h)
                    throw new InvalidDataException($"Invalid atlas rectangle for sprite '{frame.filename}'.");
            }

            var folder = $"{parent}/{Path.GetFileNameWithoutExtension(jsonAssetPath)}.PixelCraftClips";
            var tagNames = new HashSet<string>(StringComparer.Ordinal);
            var updates = new List<ClipUpdate>();
            var remainingKeys = KeyBudget;
            foreach (var tag in tags)
            {
                if (tag == null || string.IsNullOrWhiteSpace(tag.name) || !tagNames.Add(tag.name))
                    throw new InvalidDataException("Give every animation tag a unique, non-empty name.");
                var path = $"{folder}/{ClipFileName(tag.name)}.anim";
                updates.Add(new ClipUpdate
                {
                    Name = tag.name,
                    Path = path,
                    Loop = tag.repeat == 0,
                    Keys = CreateKeys(tag, sheet.frames, ref remainingKeys),
                });
            }

            // Validate the JSON before forcing a texture reimport. The matching .meta
            // carries the existing texture GUID and sprite local file IDs.
            AssetDatabase.ImportAsset(imagePath, ImportAssetOptions.ForceSynchronousImport | ImportAssetOptions.ForceUpdate);
            var texture = AssetDatabase.LoadAssetAtPath<Texture2D>(imagePath);
            var sprites = AssetDatabase.LoadAllAssetsAtPath(imagePath).OfType<Sprite>()
                .ToDictionary(sprite => sprite.name, StringComparer.Ordinal);
            if (texture == null || texture.width != size.w || texture.height != size.h || sprites.Count != sheet.frames.Length)
                throw new InvalidDataException("The Unity atlas does not match the sheet. Copy the PNG and its matching .meta.");
            var spriteFrames = new Sprite[sheet.frames.Length];
            for (var i = 0; i < sheet.frames.Length; i++)
            {
                var frame = sheet.frames[i];
                var box = frame.frame;
                if (!sprites.TryGetValue(frame.filename, out var sprite) ||
                    sprite.rect != new Rect(box.x, size.h - box.y - box.h, box.w, box.h))
                    throw new InvalidDataException($"Unity sprite '{frame.filename}' does not match the sheet rectangle.");
                spriteFrames[i] = sprite;
            }

            var ownership = "PixelCraftSheet_" + AssetDatabase.AssetPathToGUID(imagePath);
            foreach (var update in updates)
            {
                var existing = AssetDatabase.LoadMainAssetAtPath(update.Path);
                if ((existing != null && (!(existing is AnimationClip) || !AssetDatabase.GetLabels(existing).Contains(ownership))) ||
                    (existing == null && File.Exists(update.Path)))
                    throw new IOException($"An unrelated asset occupies '{update.Path}'. Move it before importing.");
                update.Existing = existing as AnimationClip;
            }

            if (!AssetDatabase.IsValidFolder(folder) &&
                (File.Exists(folder) || AssetDatabase.GUIDToAssetPath(AssetDatabase.CreateFolder(parent, Path.GetFileName(folder))) != folder))
                throw new IOException($"Could not create '{folder}'.");
            foreach (var update in updates)
            {
                var clip = update.Existing ?? new AnimationClip { name = update.Name };
                clip.frameRate = 1000;
                var curve = Array.ConvertAll(update.Keys, key =>
                    new ObjectReferenceKeyframe { time = key.Time, value = spriteFrames[key.Frame] });
                AnimationUtility.SetObjectReferenceCurve(clip, SpriteCurve, curve);
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

        private static (float Time, int Frame)[] CreateKeys(Tag tag, Frame[] frames, ref int remainingKeys)
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

            var keys = new (float Time, int Frame)[(int)keyCount];
            var index = 0;
            long milliseconds = 0;
            for (var repeat = 0; repeat < repeats; repeat++)
            {
                foreach (var frame in cycle)
                {
                    keys[index++] = (milliseconds / 1000f, frame);
                    milliseconds += frames[frame].duration;
                }
            }
            // Unity adds one sampling interval to the last object-reference key.
            if (finalDuration > 1)
                keys[index] = ((milliseconds - 1) / 1000f, keys[index - 1].Frame);
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
