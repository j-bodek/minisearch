import re
from functools import lru_cache


class SnowballStemmer:
    """
    Snowball stemmer based on rules described on official snowball website
    https://snowballstem.org/algorithms/english/stemmer.html (from 19.07.2025)

    Tested on official dataset with 100% correctness
    input file - https://github.com/snowballstem/snowball-data/blob/c87231db9e26e7fbc524b7000d720fc882a5dc80/english/voc.txt
    output file - https://github.com/snowballstem/snowball-data/blob/c87231db9e26e7fbc524b7000d720fc882a5dc80/english/output.txt
    """

    VOWELS = ("a", "e", "i", "o", "u", "y")
    DOUBLES = ("bb", "dd", "ff", "gg", "mm", "nn", "pp", "rr", "tt")
    LI_ENDINGS = ("c", "d", "e", "g", "h", "k", "m", "n", "r", "t")
    EXCEPTION_WORDS = ("sky", "news", "howe", "atlas", "cosmos", "bias", "andes")

    PRE_STEM_EXCEPTIONS = {
        "skis": "ski",
        "skies": "sky",
        "idly": "idl",
        "gently": "gentl",
        "ugly": "ugli",
        "early": "earli",
        "only": "onli",
        "singly": "singl",
        "sky": "sky",
        "news": "news",
        "howe": "howe",
    }

    R1_BEGININGS_REGEX = r"^(gener|commun|arsen|past|univers|later|emerg|organ)"

    STEP_1A_SUFFIX_REGEX = r"(sses|ied|ies|us|ss|s)$"

    STEP_2_SUFFIX_MAP = {
        "ization": "ize",
        "ational": "ate",
        "fulness": "ful",
        "ousness": "ous",
        "iveness": "ive",
        "tional": "tion",
        "biliti": "ble",
        "lessli": "less",
        "entli": "ent",
        "ation": "ate",
        "alism": "al",
        "aliti": "al",
        "ousli": "ous",
        "iviti": "ive",
        "ogist": "og",
        "fulli": "ful",
        "enci": "ence",
        "anci": "ance",
        "abli": "able",
        "izer": "ize",
        "ator": "ate",
        "alli": "al",
        "bli": "ble",
        "ogi": "og",
        "li": "",
    }

    STEP_2_SUFFIX_REGEX = r"(ization|ational|fulness|ousness|iveness|tional|biliti|lessli|entli|ation|alism|aliti|ousli|iviti|ogist|fulli|enci|anci|abli|izer|ator|alli|bli|ogi|li)$"

    STEP_3_SUFFIX_MAP = {
        "ational": "ate",
        "tional": "tion",
        "alize": "al",
        "icate": "ic",
        "iciti": "ic",
        "ative": "",
        "ical": "ic",
        "ness": "",
        "ful": "",
    }

    STEP_3_SUFFIX_REGEX = r"(ational|tional|alize|icate|iciti|ative|ical|ness|ful)$"

    STEP_4_SUFFIX_REGEX = r"(ement|ance|ence|able|ible|ment|ant|ent|ism|ate|iti|ous|ive|ize|ion|al|er|ic)$"

    def __init__(self):
        self._r1 = float("inf")
        self._r2 = float("inf")

    @lru_cache(maxsize=1024)
    def stem(self, word):
        """
        Stem the word if it has more than two characters,
        otherwise return it as is.
        """

        if len(word) <= 2 or word in self.__class__.EXCEPTION_WORDS:
            return word

        self._r1, self._r2 = len(word), len(word)

        word = self.remove_initial_apostrophe(word)
        if word in self.__class__.PRE_STEM_EXCEPTIONS:
            return self.__class__.PRE_STEM_EXCEPTIONS[word]

        word = self.set_ys(word)
        self.find_r1r2(word)

        word = self.step_0(word)
        word = self.step_1a(word)
        word = self.step_1b(word)
        word = self.step_1c(word)
        word = self.step_2(word)
        word = self.step_3(word)
        word = self.step_4(word)
        word = self.step_5(word)
        word = word.replace("Y", "y")

        return word

    def find_r1r2(self, word):
        length = len(word)
        self._r1, self._r2 = length, length

        if prefix := re.findall(self.__class__.R1_BEGININGS_REGEX, word):
            self._r1 = len(prefix[0])

            for index, match in enumerate(
                re.finditer("[aeiouy][^aeiouy]", word[self._r1 :])
            ):
                self._r2 = self._r1 + match.end()
                break

        else:
            for index, match in enumerate(re.finditer("[aeiouy][^aeiouy]", word)):
                if index == 0:
                    if match.end() < length:
                        self._r1 = match.end()
                if index == 1:
                    if match.end() < length:
                        self._r2 = match.end()
                    break

    def remove_initial_apostrophe(self, word):
        if word[0] == "'":
            word = word[1:]

        return word

    def set_ys(self, word):

        if word[0] == "y":
            word = "Y" + word[1:]

        for match in re.finditer("[aeiou]y", word):
            y_index = match.end() - 1
            char_list = [x for x in word]
            char_list[y_index] = "Y"
            word = "".join(char_list)

        return word

    def ends_with_short_syllabe(self, word):
        if word == "past":
            return True
        elif len(word) > 2:
            ending = word[len(word) - 3 :]
            if re.match("[^aeiouy][aeiouy][^aeiouwxY]", ending):
                return True
        else:
            if re.match("[aeiouy][^aeiouy]", word):
                return True

        return False

    def is_short(self, word):
        length = len(word)

        if self._r1 >= length:
            return self.ends_with_short_syllabe(word)

        return False

    def step_0(self, word):

        if word.endswith("'s'"):
            return word[:-3]
        elif word.endswith("'s"):
            return word[:-2]
        elif word.endswith("'"):
            return word[:-1]
        else:
            return word

    def step_1a(self, word):
        if suffix := re.findall(self.__class__.STEP_1A_SUFFIX_REGEX, word):
            suffix = suffix[0]

            if suffix == "sses":
                return word[:-2]
            elif suffix in ("ied", "ies"):
                word = word[:-3]

                if len(word) > 1:
                    word += "i"
                else:
                    word += "ie"
                return word
            elif suffix in ("us", "ss"):
                return word
            elif suffix == "s":
                if re.search(r"[aeiouy]+", word[:-2]):
                    return word[:-1]

        return word

    def step_1b(self, word):

        if suffix := re.findall(r"(eedly|eed)$", word):
            suffix = suffix[0]

            if len(word) - len(suffix) >= self._r1 and not any(
                (word.startswith(s) for s in ("proc", "exc", "succ"))
            ):
                word = word[: -len(suffix)] + "ee"

            return word

        elif suffix := re.findall(r"(ingly|edly|ing|ed)$", word):
            suffix = suffix[0]

            # special case for 'ing'
            if suffix == "ing":
                if re.match("^[^aeiouy]y$", word[:-3]):
                    return word[:-4] + "ie"
                elif word[:-3] in ("inn", "out", "cann", "herr", "earr", "even"):
                    return word

            if re.search(r"[aeiouy]+", word[: -len(suffix)]):
                # delete suffix
                word = word[: -len(suffix)]

                if word[-2:] in ("at", "bl", "iz"):
                    return word + "e"
                elif word[-2:] in self.__class__.DOUBLES and not (
                    len(word) == 3 and word[0] in ("a", "e", "o")
                ):
                    return word[:-1]
                elif self.is_short(word):
                    return word + "e"

        return word

    def step_1c(self, word):
        if len(word) > 2 and word[-1] in "yY" and word[-2] not in self.__class__.VOWELS:
            return word[:-1] + "i"

        return word

    def step_2(self, word):

        if suffix := re.findall(self.__class__.STEP_2_SUFFIX_REGEX, word):
            suffix, repl = suffix[0], self.__class__.STEP_2_SUFFIX_MAP[suffix[0]]

            if not (len(word) - len(suffix) >= self._r1):
                return word

            if suffix == "ogi" and word[-4] == "l":
                return word[: -len(suffix)] + repl
            elif suffix == "li" and word[-3] in self.__class__.LI_ENDINGS:
                return word[: -len(suffix)]
            elif suffix not in ["ogi", "li"]:
                return word[: -len(suffix)] + repl

        return word

    def step_3(self, word):

        if suffix := re.findall(self.__class__.STEP_3_SUFFIX_REGEX, word):
            suffix, repl = suffix[0], self.__class__.STEP_3_SUFFIX_MAP[suffix[0]]

            if word.endswith(suffix):

                if not (len(word) - len(suffix) >= self._r1):
                    return word

                if suffix == "ative" and len(word) - len(suffix) >= self._r2:
                    return word[: -len(suffix)] + repl
                elif suffix != "ative":
                    return word[: -len(suffix)] + repl

        return word

    def step_4(self, word):
        if suffix := re.findall(self.__class__.STEP_4_SUFFIX_REGEX, word):
            suffix = suffix[0]

            # r2 in suffix
            if not (len(word) - len(suffix) >= self._r2):
                return word

            if suffix == "ion" and word[-4] in "st":
                return word[: -len(suffix)]
            elif suffix != "ion":
                return word[: -len(suffix)]

        return word

    def step_5(self, word):

        if word.endswith("e"):
            if len(word) - 1 >= self._r2:
                return word[:-1]
            elif len(word) - 1 >= self._r1 and not self.ends_with_short_syllabe(
                word[:-1]
            ):
                return word[:-1]

        elif word.endswith("l") and len(word) - 1 >= self._r2 and word.endswith("ll"):
            return word[:-1]

        return word
