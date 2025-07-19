import re
from .stemmer import SnowballStemmer
from typing import Generator, ItemsView
from collections import defaultdict


class Tokenizer:

    STOP_WORDS = {
        "a",
        "and",
        "are",
        "as",
        "at",
        "be",
        "but",
        "by",
        "for",
        "if",
        "in",
        "into",
        "is",
        "it",
        "no",
        "not",
        "of",
        "on",
        "or",
        "s",
        "such",
        "t",
        "that",
        "the",
        "their",
        "then",
        "there",
        "these",
        "they",
        "this",
        "to",
        "was",
        "will",
        "with",
        "www",
    }

    def __init__(self):
        self._stemmer = SnowballStemmer()

    def tokenize(self, doc: str) -> Generator[str, None, None]:
        for token in re.sub("[^A-Za-z0-9\s]+", "", doc).lower().split():
            if token in self.__class__.STOP_WORDS:
                continue

            yield self._stemmer.stem(token)
            # yield self._stemmer.stemWord(token)

    def tokenize_group(self, doc: str) -> tuple[int, ItemsView[str, list[int]]]:
        tokens = defaultdict(list)

        i = 0
        for i, token in enumerate(self.tokenize(doc)):
            tokens[token].append(i)

        return i + 1, tokens.items()
