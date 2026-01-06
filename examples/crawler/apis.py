import flamepy
from dataclasses import dataclass


@dataclass
class WebPage:
    url: str


@dataclass
class Summary:
    links: list[str]
    content: str
