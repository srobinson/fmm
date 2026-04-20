package com.example.ui

import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch

data class UiState(
    val isLoading: Boolean = false,
    val items: List<String> = emptyList(),
    val error: String? = null
)

sealed class UiEvent {
    object Refresh : UiEvent()
    data class ItemClicked(val id: String) : UiEvent()
}

interface ViewModelContract {
    val state: StateFlow<UiState>
    fun onEvent(event: UiEvent)
}

class MainViewModel : ViewModelContract {
    override val state: StateFlow<UiState> = MutableStateFlow(UiState())

    override fun onEvent(event: UiEvent) {}

    private fun loadItems() {}
}

object ViewModelFactory {
    fun create(): MainViewModel = MainViewModel()
}
