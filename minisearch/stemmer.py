import re


class SnowballStemmer:
    """
    Snowball stemmer based on rules described on official snowball website
    https://snowballstem.org/algorithms/english/stemmer.html (from 19.07.2025)

    Tested on official dataset with 100% correctness
    input file - https://github.com/snowballstem/snowball-data/blob/c87231db9e26e7fbc524b7000d720fc882a5dc80/english/voc.txt
    output file - https://github.com/snowballstem/snowball-data/blob/c87231db9e26e7fbc524b7000d720fc882a5dc80/english/output.txt
    """

    vowels = ["a", "e", "i", "o", "u", "y"]
    doubles = ["bb", "dd", "ff", "gg", "mm", "nn", "pp", "rr", "tt"]
    li_endings = ["c", "d", "e", "g", "h", "k", "m", "n", "r", "t"]
    exceptions = ["sky", "news", "howe", "atlas", "cosmos", "bias", "andes"]
    r1_beginings = [
        "gener",
        "commun",
        "arsen",
        "past",
        "univers",
        "later",
        "emerg",
        "organ",
    ]

    pre_stem_exceptions = {
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

    def __init__(self):
        self._r1 = float("inf")
        self._r2 = float("inf")

    def stem(self, word):
        """
        Stem the word if it has more than two characters,
        otherwise return it as is.
        """

        if len(word) <= 2 or word in self.__class__.exceptions:
            return word

        self._r1, self._r2 = len(word), len(word)

        word = self.remove_initial_apostrophe(word)
        if word in self.__class__.pre_stem_exceptions:
            return self.__class__.pre_stem_exceptions[word]

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

        self._r1, self._r2 = len(word), len(word)

        found = False
        for prefix in self.__class__.r1_beginings:
            if word.startswith(prefix):
                self._r1 = len(prefix)

                for index, match in enumerate(
                    re.finditer("[aeiouy][^aeiouy]", word[len(prefix) :])
                ):
                    self._r2 = len(prefix) + match.end()
                    break

                found = True
                break

        if not found:
            for index, match in enumerate(re.finditer("[aeiouy][^aeiouy]", word)):
                if index == 0:
                    if match.end() < length:
                        self._r1 = match.end()
                if index == 1:
                    if match.end() < length:
                        self._r2 = match.end()
                    break

        self._r1 = min(self._r1, length)
        self._r2 = min(self._r2, length)

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
        if word.endswith("sses"):
            return word[:-2]
        elif word.endswith("ied") or word.endswith("ies"):
            word = word[:-3]
            # small mistake here, if 0 none will be done
            if len(word) > 1:
                word += "i"
            else:
                word += "ie"
            return word
        elif word.endswith("us") or word.endswith("ss"):
            return word
        elif word.endswith("s"):
            for letter in word[:-2]:
                if letter in self.__class__.vowels:
                    return word[:-1]

        return word

    def step_1b(self, word):
        for suffix in ["eedly", "eed"]:
            if not word.endswith(suffix):
                continue

            # mistake here, only checked if r1 existed
            if len(word) - len(suffix) >= self._r1 and not any(
                (word.startswith(s) for s in ("proc", "exc", "succ"))
            ):
                word = word[: -len(suffix)] + "ee"

            return word

        for suffix in ["ed", "edly", "ing", "ingly"]:
            if not word.endswith(suffix):
                continue

            # special case for 'ing'
            if suffix == "ing":
                if re.match("^[^aeiouy]y$", word[:-3]):
                    return word[:-4] + "ie"
                elif word[:-3] in ["inn", "out", "cann", "herr", "earr", "even"]:
                    return word

            has_vowel = False
            for l in word[: -len(suffix)]:
                if l in self.__class__.vowels:
                    has_vowel = True
                    break

            if has_vowel:
                # delete suffix
                word = word[: -len(suffix)]

                if word[-2:] in ["at", "bl", "iz"]:
                    word += "e"
                elif word[-2:] in self.__class__.doubles and not (
                    # lack of this condition
                    len(word) == 3
                    and word[0] in ["a", "e", "o"]
                ):
                    word = word[:-1]
                elif self.is_short(word):
                    word += "e"

            break

        return word

    def step_1c(self, word):
        if len(word) > 2 and word[-1] in "yY" and word[-2] not in self.__class__.vowels:
            return word[:-1] + "i"

        return word

    def step_2(self, word):
        suffixes = (
            ("ization", "ize"),
            ("ational", "ate"),
            ("fulness", "ful"),
            ("ousness", "ous"),
            ("iveness", "ive"),
            ("tional", "tion"),
            ("biliti", "ble"),
            ("lessli", "less"),
            ("entli", "ent"),
            ("ation", "ate"),
            ("alism", "al"),
            ("aliti", "al"),
            ("ousli", "ous"),
            ("iviti", "ive"),
            ("ogist", "og"),
            ("fulli", "ful"),
            ("enci", "ence"),
            ("anci", "ance"),
            ("abli", "able"),
            ("izer", "ize"),
            ("ator", "ate"),
            ("alli", "al"),
            ("bli", "ble"),
            ("ogi", "og"),
            ("li", ""),
        )

        for suffix, repl in suffixes:
            if word.endswith(suffix):

                if not (len(word) - len(suffix) >= self._r1):
                    return word

                if suffix == "ogi" and word[-4] == "l":
                    return word[: -len(suffix)] + repl
                elif suffix == "li" and word[-3] in self.__class__.li_endings:
                    return word[: -len(suffix)]
                elif suffix not in ["ogi", "li"]:
                    return word[: -len(suffix)] + repl

        return word

    def step_3(self, word):
        suffixes = (
            ("ational", "ate"),
            ("tional", "tion"),
            ("alize", "al"),
            ("icate", "ic"),
            ("iciti", "ic"),
            ("ative", ""),
            ("ical", "ic"),
            ("ness", ""),
            ("ful", ""),
        )

        for suffix, repl in suffixes:
            if word.endswith(suffix):

                if not (len(word) - len(suffix) >= self._r1):
                    return word

                if suffix == "ative" and len(word) - len(suffix) >= self._r2:
                    return word[: -len(suffix)] + repl
                elif suffix != "ative":
                    return word[: -len(suffix)] + repl

        return word

    def step_4(self, word):
        suffixes = (
            "ement",
            "ance",
            "ence",
            "able",
            "ible",
            "ment",
            "ant",
            "ent",
            "ism",
            "ate",
            "iti",
            "ous",
            "ive",
            "ize",
            "ion",
            "al",
            "er",
            "ic",
        )

        for suffix in suffixes:
            if word.endswith(suffix):

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
