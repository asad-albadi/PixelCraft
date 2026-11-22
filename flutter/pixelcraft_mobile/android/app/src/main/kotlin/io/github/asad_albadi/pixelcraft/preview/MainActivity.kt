package io.github.asad_albadi.pixelcraft.preview

import android.app.Activity
import android.content.Intent
import android.os.Handler
import android.os.Looper
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.concurrent.Executors

class MainActivity : FlutterActivity() {
    private val io = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private var pending: MethodChannel.Result? = null
    private var copyBytes: ByteArray? = null
    private val maxBytes = 8 * 1024 * 1024
    override fun configureFlutterEngine(engine: FlutterEngine) {
        super.configureFlutterEngine(engine)
        MethodChannel(engine.dartExecutor.binaryMessenger, "pixelcraft/storage").setMethodCallHandler { call, result ->
            when (call.method) {
                "recoveryRead" -> work(result) {
                    val file = File(filesDir, "recovery.snapshot")
                    if (file.exists()) readBounded(file.inputStream()) else null
                }
                "recoveryWrite" -> work(result) {
                    val bytes = call.argument<ByteArray>("bytes") ?: error("Missing recovery bytes")
                    require(bytes.size <= maxBytes) { "Recovery exceeds the mobile file budget" }
                    publish(File(filesDir, "recovery.snapshot"), bytes)
                    null
                }
                "recoveryClear" -> work(result) {
                    val file = File(filesDir, "recovery.snapshot")
                    check(!file.exists() || file.delete()) { "Could not discard recovery" }
                    null
                }
                "save" -> work(result) {
                    val bytes = call.argument<ByteArray>("bytes") ?: error("Missing project bytes")
                    require(bytes.size <= maxBytes) { "Project exceeds the preview file limit" }
                    val key = call.argument<String>("key") ?: UUID.randomUUID().toString()
                    val destination = project(key)
                    val temporary = File.createTempFile(".pixelcraft-", ".tmp", destination.parentFile)
                    try {
                        FileOutputStream(temporary).use { stream -> stream.write(bytes); stream.fd.sync() }
                        check(temporary.renameTo(destination)) { "Could not publish saved project" }
                        check(getSharedPreferences("projects", MODE_PRIVATE).edit().putString("last", key).commit()) { "Could not record saved project" }
                    } finally { temporary.delete() }
                    key
                }
                "reopen" -> work(result) {
                    val key = call.argument<String>("key") ?: getSharedPreferences("projects", MODE_PRIVATE).getString("last", null) ?: error("Save a project first")
                    mapOf("key" to key, "bytes" to readBounded(project(key).inputStream()))
                }
                "openDocument", "saveCopy" -> {
                    if (pending != null) { result.error("busy", "A file picker is already open", null); return@setMethodCallHandler }
                    pending = result
                    val save = call.method == "saveCopy"
                    copyBytes = if (save) call.argument<ByteArray>("bytes") else null
                    if (save && (copyBytes == null || copyBytes!!.size > maxBytes)) {
                        pending = null; copyBytes = null; result.error("size", "Project exceeds the preview file limit", null); return@setMethodCallHandler
                    }
                    try {
                        val intent = Intent(if (save) Intent.ACTION_CREATE_DOCUMENT else Intent.ACTION_OPEN_DOCUMENT).apply {
                            addCategory(Intent.CATEGORY_OPENABLE)
                            type = if (save) "application/octet-stream" else "*/*"
                            if (save) putExtra(Intent.EXTRA_TITLE, "PixelCraft.pixel")
                        }
                        @Suppress("DEPRECATION")
                        startActivityForResult(intent, if (save) 9002 else 9001)
                    } catch (error: Exception) {
                        pending = null; copyBytes = null; result.error("picker", error.message, null)
                    }
                }
                else -> result.notImplemented()
            }
        }
    }
    private fun publish(destination: File, bytes: ByteArray) {
        val temporary = File.createTempFile(".pixelcraft-", ".tmp", destination.parentFile)
        try {
            FileOutputStream(temporary).use { stream -> stream.write(bytes); stream.fd.sync() }
            check(temporary.renameTo(destination)) { "Could not publish recovery" }
        } finally { temporary.delete() }
    }
    private fun project(key: String): File {
        require(Regex("[a-fA-F0-9-]{36}").matches(key)) { "Invalid project identity" }
        val directory = File(filesDir, "projects").apply { mkdirs() }
        return File(directory, "$key.pixel")
    }
    private fun readBounded(input: java.io.InputStream): ByteArray = input.use { stream ->
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(32768)
        while (true) {
            val count = stream.read(buffer)
            if (count < 0) break
            require(output.size() + count <= maxBytes) { "File exceeds the preview limit" }
            output.write(buffer, 0, count)
        }
        output.toByteArray()
    }
    private fun work(result: MethodChannel.Result, action: () -> Any?) {
        io.execute {
            try { val value = action(); main.post { result.success(value) } }
            catch (error: Exception) { main.post { result.error("storage", error.message ?: "File operation failed", null) } }
        }
    }
    @Deprecated("Android document picker result")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != 9001 && requestCode != 9002) return
        val result = pending ?: return
        pending = null
        val bytes = copyBytes; copyBytes = null
        val uri = data?.data
        if (resultCode != Activity.RESULT_OK || uri == null) { result.success(null); return }
        work(result) {
            if (requestCode == 9001) {
                mapOf("bytes" to readBounded(contentResolver.openInputStream(uri) ?: error("Cannot read this document")))
            } else {
                contentResolver.openOutputStream(uri, "w").use { stream ->
                    check(stream != null) { "Cannot write this document" }
                    stream.write(bytes ?: error("Missing export bytes")); stream.flush()
                }
                // A provider may expose an existing document. Never delete it on a write failure.
                true
            }
        }
    }
    override fun onDestroy() {
        pending?.error("closed", "File picker closed with the editor", null)
        pending = null; copyBytes = null
        io.shutdown()
        super.onDestroy()
    }
}
