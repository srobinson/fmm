
defmodule MyApp.Cache do
  use GenServer

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  def get(key) do
    GenServer.call(__MODULE__, {:get, key})
  end

  def put(key, value) do
    GenServer.cast(__MODULE__, {:put, key, value})
  end

  defp init(opts) do
    {:ok, %{}}
  end

  defp handle_call({:get, key}, _from, state) do
    {:reply, Map.get(state, key), state}
  end

  defp handle_cast({:put, key, value}, state) do
    {:noreply, Map.put(state, key, value)}
  end
end
