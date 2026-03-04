// Sample Kotlin file for fmm parser validation
// Covers: classes, interfaces, objects, data classes, sealed classes,
// enum classes, functions, properties, typealiases, imports, visibility

package com.example.app

import kotlin.collections.List
import kotlin.collections.Map
import java.util.UUID
import java.util.concurrent.Executor
import org.example.utils.Logger

// Public classes (default visibility = public)

class NetworkManager {
    fun fetchData(): ByteArray? = null

    private fun internalRetry() {}

    fun cancelAll() {}
}

open class BaseRepository {
    open fun findById(id: String): Any? = null
}

// Data classes

data class UserProfile(
    val name: String,
    val email: String,
    val age: Int
)

data class APIResponse<T>(
    val data: T?,
    val error: String?
)

// Sealed class

sealed class Result<out T> {
    data class Success<T>(val data: T) : Result<T>()
    data class Failure(val error: Throwable) : Result<Nothing>()
    object Loading : Result<Nothing>()
}

// Interfaces

interface Repository<T> {
    fun findAll(): List<T>
    fun save(item: T): Boolean
}

interface Cacheable {
    fun invalidateCache()
}

// Objects

object AppConfig {
    val baseURL = "https://api.example.com"
    val timeout = 30_000L
}

object DatabaseManager {
    fun getConnection(): Any = TODO()
}

// Enum class

enum class Direction {
    NORTH, SOUTH, EAST, WEST
}

enum class HttpStatus(val code: Int) {
    OK(200),
    NOT_FOUND(404),
    SERVER_ERROR(500)
}

// Private and internal (should NOT be exported)

private class InternalHelper {
    fun process() {}
}

internal class ModuleInternal {
    fun doWork() {}
}

private fun hiddenFunction(): String = "hidden"
internal fun moduleFunction(): Int = 42

// Top-level public declarations

fun createManager(config: Map<String, Any>): NetworkManager = NetworkManager()

fun processData(input: List<String>): List<String> = input.map { it.uppercase() }

val MAX_RETRIES = 3
var isDebugMode = false
val VERSION = "1.0.0"

typealias StringMap = Map<String, String>
typealias Callback<T> = (T) -> Unit

// Class with companion object

class ServiceLocator {
    companion object {
        const val TAG = "ServiceLocator"
        fun getInstance(): ServiceLocator = ServiceLocator()
    }

    fun resolve(name: String): Any? = null
}

// Annotated declarations

@Deprecated("Use newMethod instead")
fun oldMethod(): Unit {}

suspend fun asyncOperation(): String = "result"
