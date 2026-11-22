package io.github.asad_albadi.pixelcraft.preview

import java.util.Locale

/** Validates both the suggested SAF name and the private cache basename. */
internal object ExportPolicy {
    const val MAX_BYTES = 8 * 1024 * 1024
    data class Document(val bytes: ByteArray, val name: String, val mime: String)

    fun requireContentScheme(scheme: String?) {
        require(scheme == "content") { "Document provider must return a content URI" }
    }

    fun validate(bytes: ByteArray?, name: String?, mime: String?): Document {
        require(bytes != null && bytes.size <= MAX_BYTES) { "Missing export bytes or export exceeds 8 MiB" }
        require(name != null && name.length <= 120 &&
            Regex("[A-Za-z0-9][A-Za-z0-9 _().-]*").matches(name) &&
            !name.contains("..") && !name.endsWith(" ")) { "Invalid export filename" }
        val extension = name.substringAfterLast('.', "").lowercase(Locale.ROOT)
        val allowed = when (extension) {
            "png" -> setOf("image/png")
            "gif" -> setOf("image/gif")
            "ase", "aseprite" -> setOf("application/octet-stream", "application/x-aseprite", "image/x-aseprite")
            "pixel" -> setOf("application/octet-stream", "application/x-pixelcraft")
            "json" -> setOf("application/json")
            else -> emptySet()
        }
        require(mime in allowed) { "Unsupported export extension or MIME type" }
        return Document(bytes, name, requireNotNull(mime))
    }
}
