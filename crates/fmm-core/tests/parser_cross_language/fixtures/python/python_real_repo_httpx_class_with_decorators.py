
import typing
from ._utils import primitive_value_to_str

class QueryParams(typing.Mapping[str, str]):
    def __init__(self, *args: typing.Any, **kwargs: typing.Any) -> None:
        self._dict: dict = {}

    @property
    def multi_items(self) -> typing.List[typing.Tuple[str, str]]:
        return list(self._dict.items())

    @staticmethod
    def _coerce(value: typing.Any) -> str:
        return primitive_value_to_str(value)

    def keys(self) -> typing.List[str]:
        return list(self._dict.keys())
