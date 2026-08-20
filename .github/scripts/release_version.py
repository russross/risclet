#!/usr/bin/env python3

import argparse
import re
from dataclasses import dataclass


SEMANTIC_VERSION_PATTERN = re.compile(
    r"^(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)

PreReleaseIdentifier = int | str


@dataclass(frozen=True, slots=True)
class Arguments:
    previous: str
    current: str


@dataclass(frozen=True, slots=True)
class SemanticVersion:
    major: int
    minor: int
    patch: int
    prerelease: tuple[PreReleaseIdentifier, ...]

    @classmethod
    def parse(cls, value: str) -> "SemanticVersion":
        match = SEMANTIC_VERSION_PATTERN.fullmatch(value)
        if match is None:
            raise ValueError(f"invalid semantic version: {value}")

        prerelease_text = match.group(4)
        prerelease = (
            tuple(
                cls._parse_prerelease_identifier(part)
                for part in prerelease_text.split(".")
            )
            if prerelease_text is not None
            else ()
        )
        return cls(
            major=int(match.group(1)),
            minor=int(match.group(2)),
            patch=int(match.group(3)),
            prerelease=prerelease,
        )

    @staticmethod
    def _parse_prerelease_identifier(value: str) -> PreReleaseIdentifier:
        if not value.isdigit():
            return value
        if len(value) > 1 and value.startswith("0"):
            raise ValueError(
                f"numeric prerelease identifier has a leading zero: {value}"
            )
        return int(value)

    def has_higher_precedence_than(self, other: "SemanticVersion") -> bool:
        own_core = (self.major, self.minor, self.patch)
        other_core = (other.major, other.minor, other.patch)
        if own_core != other_core:
            return own_core > other_core
        return self._compare_prerelease(other) > 0

    def _compare_prerelease(self, other: "SemanticVersion") -> int:
        if not self.prerelease:
            return int(bool(other.prerelease))
        if not other.prerelease:
            return -1

        for own_identifier, other_identifier in zip(
            self.prerelease, other.prerelease, strict=False
        ):
            comparison = self._compare_identifier(own_identifier, other_identifier)
            if comparison != 0:
                return comparison
        return (len(self.prerelease) > len(other.prerelease)) - (
            len(self.prerelease) < len(other.prerelease)
        )

    @staticmethod
    def _compare_identifier(
        own_identifier: PreReleaseIdentifier,
        other_identifier: PreReleaseIdentifier,
    ) -> int:
        if isinstance(own_identifier, int) and isinstance(other_identifier, str):
            return -1
        if isinstance(own_identifier, str) and isinstance(other_identifier, int):
            return 1
        return (own_identifier > other_identifier) - (
            own_identifier < other_identifier
        )


def validate_increase(previous_value: str, current_value: str) -> None:
    previous = SemanticVersion.parse(previous_value)
    current = SemanticVersion.parse(current_value)
    if not current.has_higher_precedence_than(previous):
        raise ValueError(
            f"Cargo package version must increase: {previous_value} -> {current_value}"
        )


def parse_args() -> Arguments:
    parser = argparse.ArgumentParser(
        description="Validate a Cargo package version transition."
    )
    parser.add_argument("previous", help="version before the change")
    parser.add_argument("current", help="version after the change")
    namespace = parser.parse_args()
    return Arguments(previous=namespace.previous, current=namespace.current)


def main() -> None:
    args = parse_args()
    try:
        validate_increase(args.previous, args.current)
    except ValueError as error:
        raise SystemExit(f"error: {error}") from None


if __name__ == "__main__":
    main()
