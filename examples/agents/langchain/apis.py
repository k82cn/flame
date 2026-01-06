from dataclasses import dataclass


@dataclass
class SysPrompt:
    prompt: str


@dataclass
class Question:
    question: str


@dataclass
class Answer:
    answer: str
