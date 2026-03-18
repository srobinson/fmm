defmodule MyApp.Router do
  use Plug.Router
  import Logger
  alias MyApp.{Controller, Service}
  require EEx

  @behaviour Plug

  def handle(conn, _opts) do
    send_resp(conn, 200, "OK")
  end

  defp private_handler(conn) do
    conn
  end

  defmacro route(method, path) do
    quote do
      unquote(method)(unquote(path))
    end
  end

  defmacrop private_macro() do
    quote do: :ok
  end

  defguard is_valid(x) when is_integer(x) and x > 0

  defguardp is_internal(x) when is_atom(x)

  defdelegate format(data), to: Formatter

  defstruct [:name, :age]
end

defprotocol Printable do
  @doc "Prints the value"
  def print(value)
end

defimpl Printable, for: Integer do
  def print(value), do: IO.puts(value)
end

defmodule MyApp.Helpers do
  use GenServer
  import Enum
  alias MyApp.Utils

  def helper_function() do
    :ok
  end

  def another_helper(arg) do
    arg
  end

  defp internal_work() do
    :private
  end
end

defmodule MyApp.Config do
  @moduledoc "Configuration module"

  def get(key) do
    Application.get_env(:my_app, key)
  end

  def set(key, value) do
    Application.put_env(:my_app, key, value)
  end
end
