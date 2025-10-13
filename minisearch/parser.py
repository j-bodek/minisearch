import re


class QueryParser:

    def parse_slop(self, query: str):
        slop = 0
        if match := re.match(r"^[\"]+(.*)[\"]+~([0-9]+)$", query):
            query, slop = match.group(1), int(match.group(2))

        query = query.strip('"')
        return query, slop

    def parse_fuzziness(self, query: str):
        for m in re.finditer(r"([^~\s]+)((~)([0-9]*)|[\s]+)", query):
            word, fuzziness, distance = m.group(1), m.group(3), m.group(4)
            distance = int(distance) if distance else (-1 if fuzziness else 0)
            yield (word, distance)
