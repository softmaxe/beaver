"""Catalog parity checks.

The web front-end had an equivalent test that grepped translation keys out of
its JavaScript; this is the same guarantee against a plain Python dict.
"""

import re

import pytest

from rename_subtitles.i18n import CATALOG, DEFAULT_LANGUAGE, LANGUAGES, Translator

PLACEHOLDER = re.compile(r"{(\w+)}")


def test_catalog_holds_exactly_the_declared_languages():
    assert set(CATALOG) == set(LANGUAGES)
    assert DEFAULT_LANGUAGE in LANGUAGES


def test_every_key_is_defined_in_both_languages():
    zh_keys = set(CATALOG["zh"])
    en_keys = set(CATALOG["en"])

    assert zh_keys - en_keys == set()
    assert en_keys - zh_keys == set()


@pytest.mark.parametrize("language", LANGUAGES)
def test_no_translation_is_blank(language: str):
    blank = [key for key, value in CATALOG[language].items() if not value.strip()]

    assert blank == []


def test_placeholders_agree_across_languages():
    mismatched = {
        key: (PLACEHOLDER.findall(CATALOG["zh"][key]), PLACEHOLDER.findall(CATALOG["en"][key]))
        for key in CATALOG["zh"]
        if set(PLACEHOLDER.findall(CATALOG["zh"][key]))
        != set(PLACEHOLDER.findall(CATALOG["en"][key]))
    }

    assert mismatched == {}


def test_every_key_is_dot_namespaced():
    unnamespaced = [key for key in CATALOG["zh"] if "." not in key]

    assert unnamespaced == []


def test_translator_formats_placeholders():
    translator = Translator("en")

    assert translator("summary.selected", selected=2, total=5) == "2 of 5 selected"


def test_translator_returns_unknown_keys_unchanged():
    assert Translator()("setup.does_not_exist") == "setup.does_not_exist"


def test_toggle_cycles_back_to_the_starting_language():
    translator = Translator()
    start = translator.language

    seen = [translator.toggle() for _ in LANGUAGES]

    assert seen[-1] == start
    assert set(seen) == set(LANGUAGES)


def test_an_unknown_language_falls_back_to_the_default():
    assert Translator("klingon").language == DEFAULT_LANGUAGE
