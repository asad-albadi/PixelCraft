package om.astralhive.pixelcraft

/** Standalone JVM checks: no emulator or Android runtime is required. */
object ExportPolicyTest {
    @JvmStatic fun main(args: Array<String>) {
        ExportPolicy.requireContentScheme("content")
        listOf(null, "file", "http", "https", "android.resource", "CONTENT").forEach { scheme ->
            check(runCatching { ExportPolicy.requireContentScheme(scheme) }.exceptionOrNull() is IllegalArgumentException)
        }
        val bytes = byteArrayOf(1, 2, 3)
        val formats = listOf(
            "Sprite.PNG" to "image/png", "animation.gif" to "image/gif",
            "project.ase" to "application/octet-stream", "project.ase" to "application/x-aseprite",
            "project.ase" to "image/x-aseprite", "PixelCraft.aseprite" to "application/octet-stream", "Project (copy).pixel" to "application/octet-stream",
            "project.pixel" to "application/x-pixelcraft", "atlas.json" to "application/json",
        )
        formats.forEach { (name, mime) ->
            val document = ExportPolicy.validate(bytes, name, mime)
            check(document.name == name && document.mime == mime && document.bytes === bytes)
        }
        fun rejected(name: String?, mime: String?, payload: ByteArray? = bytes) {
            check(runCatching { ExportPolicy.validate(payload, name, mime) }.exceptionOrNull() is IllegalArgumentException) {
                "Unexpectedly accepted $name ($mime)"
            }
        }
        listOf("../sprite.png", "dir/sprite.png", "dir\\sprite.png", ".png", "sprite..png", "/sprite.png",
            "sprite.png\n", "sprite.png ", "sprite.exe", "sprite", "a".repeat(121) + ".png").forEach {
            rejected(it, "image/png")
        }
        rejected(null, "image/png")
        rejected("sprite.png", null)
        rejected("sprite.png", "application/octet-stream")
        rejected("sprite.gif", "image/png")
        rejected("sprite.png", "image/png", null)
        rejected("sprite.png", "image/png", ByteArray(ExportPolicy.MAX_BYTES + 1))
        check(ExportPolicy.validate(ByteArray(ExportPolicy.MAX_BYTES), "max.png", "image/png").bytes.size == ExportPolicy.MAX_BYTES)
        println("ExportPolicy: valid format pairs, traversal, invalid MIME, missing arguments, and 8 MiB boundary passed")
    }
}
