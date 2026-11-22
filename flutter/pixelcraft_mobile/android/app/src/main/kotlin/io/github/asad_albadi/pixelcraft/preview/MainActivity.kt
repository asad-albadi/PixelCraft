package io.github.asad_albadi.pixelcraft.preview

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.provider.OpenableColumns
import android.provider.DocumentsContract
import android.net.Uri
import androidx.core.content.FileProvider
import android.content.res.AssetFileDescriptor
import android.os.CancellationSignal
import android.os.OperationCanceledException
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
import java.util.concurrent.ExecutorService
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.SynchronousQueue
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

class MainActivity : FlutterActivity() {
    private val io = Executors.newSingleThreadExecutor()
    // External providers cannot occupy the private project/recovery worker. All provider
    // pools have fixed thread counts and bounded queues, including cancellation IPC.
    private val providerIo = ThreadPoolExecutor(1, 1, 0L, TimeUnit.MILLISECONDS, ArrayBlockingQueue<Runnable>(1))
    private val providerReadIo = ThreadPoolExecutor(1, 1, 0L, TimeUnit.MILLISECONDS, SynchronousQueue<Runnable>())
    private val providerCancelIo = ThreadPoolExecutor(2, 2, 0L, TimeUnit.MILLISECONDS, ArrayBlockingQueue<Runnable>(2))
    private class ProviderRead(val result: MethodChannel.Result) {
        val signal = CancellationSignal()
        val canceled = AtomicBoolean(false)
        val descriptor = AtomicReference<AssetFileDescriptor?>(null)
        fun checkCanceled() {
            if (canceled.get()) throw OperationCanceledException()
            signal.throwIfCanceled()
        }
    }
    private var activeRead: ProviderRead? = null
    private val main = Handler(Looper.getMainLooper())
    private var pending: MethodChannel.Result? = null
    private var copyBytes: ByteArray? = null
    private var copyReturnName = false
    private val maxBytes = ExportPolicy.MAX_BYTES
    private val running = mutableSetOf<MethodChannel.Result>()
    private var closed = false
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
                "cancelOpenDocument" -> {
                    cancelOpenRead()
                    result.success(null)
                }
                "savedExportFolder" -> work(result, executor = providerIo) {
                    savedFolder()?.let { folderLabel(it) }
                }
                "clearExportFolder" -> work(result, executor = providerIo) {
                    val preferences = getSharedPreferences("exportFolders", MODE_PRIVATE)
                    val previous = preferences.getString("uri", null)?.let { Uri.parse(it) }
                    if (previous != null) releaseFolder(previous)
                    check(preferences.edit().remove("uri").commit()) { "Could not forget export folder" }
                    null
                }
                "chooseExportFolder" -> {
                    if (pending != null) { result.error("busy", "A file picker is already open", null); return@setMethodCallHandler }
                    pending = result
                    try {
                        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                                Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION or Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
                        }
                        @Suppress("DEPRECATION")
                        startActivityForResult(intent, 9003)
                    } catch (error: Exception) {
                        pending = null
                        result.error("picker", error.message, null)
                    }
                }
                "exportToFolder" -> {
                    val returnName = call.argument<Any>("returnName") == true
                    val export = try { ExportPolicy.validate(call.argument<ByteArray>("bytes"), call.argument<String>("name"), call.argument<String>("mime")) }
                    catch (error: Exception) { result.error("export", error.message, null); return@setMethodCallHandler }
                    work(result, executor = providerIo) {
                        val tree = savedFolder() ?: error("Choose an export folder again; its write permission is unavailable")
                        val parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
                        // createDocument creates a copy; providers choose how duplicate names are disambiguated.
                        val document = DocumentsContract.createDocument(contentResolver, parent, export.mime, export.name)
                            ?: error("Provider could not create a new export")
                        ExportPolicy.requireContentScheme(document.scheme)
                        require(document.authority == tree.authority) { "Provider returned an unexpected document authority" }
                        contentResolver.openOutputStream(document, "w").use { stream ->
                            check(stream != null) { "Cannot write this export" }
                            stream.write(export.bytes)
                            stream.flush()
                        }
                        // Preserve the provider document even if a write fails partway through.
                        exportResult(document, returnName)
                    }
                }
                "shareDocument" -> {
                    val export = try { ExportPolicy.validate(call.argument<ByteArray>("bytes"), call.argument<String>("name"), call.argument<String>("mime")) }
                    catch (error: Exception) { result.error("export", error.message, null); return@setMethodCallHandler }
                    work(result, completion = { value ->
                        val file = value as File
                        val uri = FileProvider.getUriForFile(this, "$packageName.exports", file)
                        val send = Intent(Intent.ACTION_SEND).apply {
                            type = export.mime
                            putExtra(Intent.EXTRA_STREAM, uri)
                            clipData = ClipData.newUri(contentResolver, export.name, uri)
                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        }
                        startActivity(Intent.createChooser(send, "Share ${export.name}").apply {
                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        })
                        // The chooser opened; Android does not report whether a recipient saved it.
                        result.success(true)
                    }) {
                        val directory = File(cacheDir, "exports").apply { check(mkdirs() || isDirectory) }
                        cleanupExports(directory)
                        val entry = File(directory, UUID.randomUUID().toString()).apply { check(mkdir()) }
                        val file = File(entry, export.name)
                        publish(file, export.bytes)
                        file
                    }
                }
                "openDocument", "saveCopy", "exportDocument" -> {
                    if (call.method == "openDocument" && activeRead != null) {
                        result.error("busy", "The previous document provider read is still finishing", null)
                        return@setMethodCallHandler
                    }
                    if (pending != null) { result.error("busy", "A file picker is already open", null); return@setMethodCallHandler }
                    val export = if (call.method == "exportDocument") {
                        try { ExportPolicy.validate(call.argument<ByteArray>("bytes"), call.argument<String>("name"), call.argument<String>("mime")) }
                        catch (error: Exception) { result.error("export", error.message, null); return@setMethodCallHandler }
                    } else null
                    pending = result
                    copyReturnName = call.method == "exportDocument" && call.argument<Any>("returnName") == true
                    val save = call.method != "openDocument"
                    copyBytes = if (save) export?.bytes ?: call.argument<ByteArray>("bytes") else null
                    if (save && (copyBytes == null || copyBytes!!.size > maxBytes)) {
                        pending = null; copyBytes = null; copyReturnName = false; result.error("size", "Project exceeds the preview file limit", null); return@setMethodCallHandler
                    }
                    try {
                        val intent = Intent(if (save) Intent.ACTION_CREATE_DOCUMENT else Intent.ACTION_OPEN_DOCUMENT).apply {
                            addCategory(Intent.CATEGORY_OPENABLE)
                            type = if (save) export?.mime ?: "application/octet-stream" else "*/*"
                            if (save) putExtra(Intent.EXTRA_TITLE, export?.name ?: "PixelCraft.pixel")
                        }
                        @Suppress("DEPRECATION")
                        startActivityForResult(intent, if (save) 9002 else 9001)
                    } catch (error: Exception) {
                        pending = null; copyBytes = null; copyReturnName = false; result.error("picker", error.message, null)
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
    private fun readBounded(input: java.io.InputStream, checkCanceled: () -> Unit = {}): ByteArray = input.use { stream ->
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(32768)
        while (true) {
            checkCanceled()
            val count = stream.read(buffer)
            checkCanceled()
            if (count < 0) break
            require(output.size() + count <= maxBytes) { "File exceeds the preview limit" }
            output.write(buffer, 0, count)
        }
        output.toByteArray()
    }
    // Cache exports stay available after chooser launch, including chooser/recipient failures.
    // Cleanup runs only before the next share; at most 32 exports / 64 MiB remain.
    private fun cleanupExports(directory: File) {
        val entries = directory.listFiles()?.sortedBy { it.lastModified() }?.toMutableList() ?: return
        val cutoff = System.currentTimeMillis() - 24L * 60 * 60 * 1000
        entries.filter { it.lastModified() < cutoff }.forEach { entry ->
            check(entry.deleteRecursively()) { "Could not clean old export" }
            entries.remove(entry)
        }
        fun size(): Long = entries.sumOf { entry -> entry.walkTopDown().filter { it.isFile }.sumOf { it.length() } }
        while (entries.size >= 32 || size() + maxBytes > 64L * 1024 * 1024) {
            check(entries.removeAt(0).deleteRecursively()) { "Could not clean export cache" }
        }
    }
    private fun work(result: MethodChannel.Result, completion: ((Any?) -> Unit)? = null,
                     executor: ExecutorService = io, action: () -> Any?) {
        if (closed) { result.error("closed", "Editor closed", null); return }
        running.add(result)
        try {
            executor.execute {
                try {
                    val value = action()
                    main.post {
                        if (running.remove(result)) {
                            try { if (completion == null) result.success(value) else completion(value) }
                            catch (error: Exception) { result.error("storage", error.message ?: "File operation failed", null) }
                        }
                    }
                } catch (error: Exception) {
                    main.post { if (running.remove(result)) result.error("storage", error.message ?: "File operation failed", null) }
                }
            }
        } catch (error: Exception) {
            running.remove(result)
            result.error(if (closed) "closed" else "busy", if (closed) "Editor closed" else "Document provider is busy", null)
        }
    }
    private fun cancelOpenRead() {
        val read = activeRead ?: return
        // Complete the original call on the main thread immediately. Late worker replies
        // are suppressed, while activeRead stays occupied until that worker actually exits.
        if (!read.canceled.compareAndSet(false, true)) return
        if (running.remove(read.result)) read.result.success(null)
        // Closing the descriptor must not wait behind a provider's blocked cancel IPC.
        dispatchCancellation { read.signal.cancel() }
        dispatchCancellation { read.descriptor.getAndSet(null)?.close() }
    }
    private fun dispatchCancellation(action: () -> Unit) {
        try { providerCancelIo.execute { try { action() } catch (_: Exception) { } } }
        catch (_: java.util.concurrent.RejectedExecutionException) {
            // Broken providers can retain cancellation workers. Local cancellation still
            // completes immediately; the bounded read loop/finally closes on unblocking.
        }
    }
    private fun openProviderDocument(result: MethodChannel.Result, uri: Uri) {
        if (closed) { result.error("closed", "Editor closed", null); return }
        if (activeRead != null) { result.error("busy", "Document provider is still reading", null); return }
        val read = ProviderRead(result)
        activeRead = read
        running.add(result)
        try {
            providerReadIo.execute {
                var value: Any? = null
                var failure: Exception? = null
                try {
                    read.checkCanceled()
                    val descriptor = contentResolver.openAssetFileDescriptor(uri, "r", read.signal)
                        ?: error("Cannot read this document")
                    read.descriptor.set(descriptor)
                    read.checkCanceled()
                    val bytes = readBounded(descriptor.createInputStream(), read::checkCanceled)
                    read.checkCanceled()
                    val name = documentName(uri, read.signal)
                    read.checkCanceled()
                    value = mapOf("bytes" to bytes, "name" to name)
                } catch (error: Exception) { failure = error }
                finally {
                    try { read.descriptor.getAndSet(null)?.close() }
                    catch (error: Exception) { if (failure == null) failure = error }
                }
                main.post {
                    if (activeRead === read) activeRead = null
                    if (running.remove(result)) {
                        val error = failure
                        if (read.canceled.get() || error is OperationCanceledException) result.success(null)
                        else if (error != null) result.error("storage", error.message ?: "Cannot read this document", null)
                        else result.success(value)
                    }
                }
            }
        } catch (_: java.util.concurrent.RejectedExecutionException) {
            activeRead = null
            running.remove(result)
            result.error("busy", "Document provider is busy", null)
        }
    }
    private fun savedFolder(): Uri? {
        val value = getSharedPreferences("exportFolders", MODE_PRIVATE).getString("uri", null) ?: return null
        val uri = Uri.parse(value)
        if (uri.scheme != "content" || !DocumentsContract.isTreeUri(uri)) return null
        return uri.takeIf { contentResolver.persistedUriPermissions.any { grant -> grant.uri == uri && grant.isWritePermission } }
    }
    private fun folderLabel(tree: Uri): String {
        val root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        return documentName(root)?.takeIf { it.isNotBlank() } ?: "Selected export folder"
    }
    private fun releaseFolder(uri: Uri) {
        // Release only the URI recorded by this feature, never other persisted app grants.
        val grant = contentResolver.persistedUriPermissions.firstOrNull { it.uri == uri } ?: return
        val flags = (if (grant.isReadPermission) Intent.FLAG_GRANT_READ_URI_PERMISSION else 0) or
            (if (grant.isWritePermission) Intent.FLAG_GRANT_WRITE_URI_PERMISSION else 0)
        if (flags != 0) contentResolver.releasePersistableUriPermission(uri, flags)
    }
    private fun rememberFolder(uri: Uri, returnedFlags: Int): String {
        ExportPolicy.requireContentScheme(uri.scheme)
        require(DocumentsContract.isTreeUri(uri)) { "Provider did not return a document tree" }
        val flags = returnedFlags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        require(flags and Intent.FLAG_GRANT_WRITE_URI_PERMISSION != 0 &&
            returnedFlags and Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION != 0) { "Provider did not grant persistent folder write access" }
        contentResolver.takePersistableUriPermission(uri, flags)
        check(contentResolver.persistedUriPermissions.any { it.uri == uri && it.isWritePermission }) { "Folder write permission was not persisted" }
        val preferences = getSharedPreferences("exportFolders", MODE_PRIVATE)
        val previous = preferences.getString("uri", null)?.let { Uri.parse(it) }
        if (!preferences.edit().putString("uri", uri.toString()).commit()) {
            if (previous != uri) releaseFolder(uri)
            error("Could not remember export folder")
        }
        if (previous != null && previous != uri) {
            // The new choice is already saved; a revoked old grant must not hide its success.
            try { releaseFolder(previous) } catch (_: SecurityException) { }
        }
        return folderLabel(uri)
    }
    private fun exportResult(uri: Uri, returnName: Boolean): Any {
        if (!returnName) return true
        val name = documentName(uri)?.takeIf { it.isNotBlank() }
            ?: error("Provider did not report the saved document name")
        return mapOf("name" to name)
    }
    private fun documentName(uri: android.net.Uri, signal: CancellationSignal? = null): String? = try {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null, signal)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0)?.takeIf { it.length <= 255 } else null
        }
    } catch (_: Exception) { null }
    @Deprecated("Android document picker result")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != 9001 && requestCode != 9002 && requestCode != 9003) return
        val result = pending ?: return
        pending = null
        val bytes = copyBytes; copyBytes = null
        val returnName = copyReturnName; copyReturnName = false
        val uri = data?.data
        if (resultCode != Activity.RESULT_OK || uri == null) { result.success(null); return }
        try { ExportPolicy.requireContentScheme(uri.scheme) }
        catch (error: IllegalArgumentException) { result.error("picker", error.message, null); return }
        if (requestCode == 9001) { openProviderDocument(result, uri); return }
        work(result, executor = providerIo) {
            if (requestCode == 9003) {
                rememberFolder(uri, data.flags)
            } else {
                contentResolver.openOutputStream(uri, "w").use { stream ->
                    check(stream != null) { "Cannot write this document" }
                    stream.write(bytes ?: error("Missing export bytes")); stream.flush()
                }
                // A provider may expose an existing document. Never delete it on a write failure.
                exportResult(uri, returnName)
            }
        }
    }
    override fun onDestroy() {
        closed = true
        cancelOpenRead()
        running.forEach { it.error("closed", "Editor closed during file operation", null) }
        running.clear()
        pending?.error("closed", "File picker closed with the editor", null)
        pending = null; copyBytes = null; copyReturnName = false
        io.shutdown()
        providerIo.shutdownNow()
        providerReadIo.shutdownNow()
        // The already queued cancellation must be allowed to run after destruction.
        providerCancelIo.shutdown()
        super.onDestroy()
    }
}
