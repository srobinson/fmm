
import typing

def encode_content(content: typing.Union[str, bytes]) -> typing.Tuple[bytes, str]:
    if isinstance(content, str):
        body = content.encode("utf-8")
        content_type = "text/plain; charset=utf-8"
    elif isinstance(content, bytes):
        body = content
        content_type = "application/octet-stream"
    else:
        raise TypeError(f"Unexpected type for content: {type(content)}")
    return body, content_type

def encode_urlencoded_data(data: dict) -> typing.Tuple[bytes, str]:
    from urllib.parse import urlencode
    body = urlencode(data).encode("utf-8")
    content_type = "application/x-www-form-urlencoded"
    return body, content_type
