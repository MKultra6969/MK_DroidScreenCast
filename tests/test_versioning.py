from mkdsc.versioning import _parse_version, is_newer


def test_parse_version_ignores_prefix():
    assert _parse_version("v1.2.3") == (1, 2, 3)
    assert _parse_version("1.0.2") == (1, 0, 2)
    assert _parse_version("") == ()


def test_is_newer():
    assert is_newer("1.0.2", "1.0.3")
    assert is_newer("1.0.2", "v1.1.0")
    assert not is_newer("1.0.2", "1.0.2")
    assert not is_newer("1.1.0", "1.0.9")
