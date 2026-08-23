#!/usr/bin/env python3
"""**gate2's §0 is the spec's §0, byte for byte.**

    python3 section0.py            # check
    python3 section0.py --json     # check, and print the document

Wave 4's amended instruction is *"copy §0 (as amended) into
economics-round/gate2/README.md §0 BEFORE any number"*. Copying is an act;
**staying** copied is a property, and a property nobody checks is one that
quietly stops holding the first time a number arrives that would read better
against a slightly different clause.

The extraction and the un-quoting are **`../gate/section0.py`'s**, imported
rather than re-typed: two copies of a verbatim-check would be exactly the kind
of drift the check exists to catch, and a second implementation could agree with
a second copy of the text. Only the README path differs.

The amendment is checked too, and separately: the block quote of
`docs/currency-amendment.md`'s fallback sentence must also be byte-equal to the
amendment's own line. §0 as amended is two verbatim quotes, so it is two checks.

Exit status is the verdict, taken directly and never through a pipe:

* `0` - gate2's §0 is the spec's §0 and its amendment quote is the amendment's.
* `1` - one of them differs. The document names the first line at which it does.
* `2` - the check could not run: a file is missing, or an anchor is not in it.
"""
import hashlib
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..', '..'))
README = f'{HERE}/README.md'
AMENDMENT = f'{ROOT}/docs/currency-amendment.md'
# The amendment's fallback sentence, anchored on its own opening words rather
# than on a line number.
AMENDMENT_ANCHOR = '- Fallback if U\' fails:'

_spec = importlib.util.spec_from_file_location(
    'gate_section0', f'{HERE}/../gate/section0.py')
gate_section0 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate_section0)


def quoted_block(text, anchor):
    """The first `> `-quoted block whose first line starts with `anchor`."""
    lines = text.split('\n')
    start = next((i for i, line in enumerate(lines)
                  if line.startswith('> ' + anchor)), None)
    if start is None:
        raise LookupError(f'no quoted line starts with {anchor!r}')
    out = []
    for line in lines[start:]:
        if line == '>':
            out.append('')
        elif line.startswith('> '):
            out.append(line[2:])
        else:
            break
    while out and not out[-1].strip():
        out.pop()
    return '\n'.join(out)


def unwrapped(text):
    """A markdown paragraph's words, so a re-wrap is not a difference.

    The amendment's fallback is **one line** in its source file and cannot be
    quoted as one line in a README without a 400-column row. Re-wrapping is the
    only transformation this check forgives, and it forgives it by comparing the
    whitespace-normalised word sequences - so a changed word, a dropped clause
    or a moved number is still a difference, and only the line breaks are not.
    """
    return ' '.join(text.split())


def main():
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-gate2-section0-verbatim',
        'spec': gate_section0.SPEC,
        'amendment': AMENDMENT,
        'readme': README,
        'anchor': gate_section0.HEADING,
    }
    try:
        with open(gate_section0.SPEC, encoding='utf-8') as handle:
            spec_text = handle.read()
        with open(README, encoding='utf-8') as handle:
            readme_text = handle.read()
        with open(AMENDMENT, encoding='utf-8') as handle:
            amendment_text = handle.read()
        section = gate_section0.spec_section0(spec_text)
        copied = quoted_block(readme_text, gate_section0.HEADING)
        fallback_copy = quoted_block(readme_text, AMENDMENT_ANCHOR)
        fallback_source = next(
            (line for line in amendment_text.split('\n')
             if line.startswith(AMENDMENT_ANCHOR)), None)
        if fallback_source is None:
            raise LookupError(f'{AMENDMENT}: no line starts with '
                              f'{AMENDMENT_ANCHOR!r}')
    except (OSError, LookupError) as error:
        document['error'] = f'{error}'
        document['SECTION0_VERBATIM'] = False
        print(json.dumps(document, indent=1, ensure_ascii=False))
        return 2

    document['specSha256'] = hashlib.sha256(spec_text.encode()).hexdigest()
    document['sectionSha256'] = hashlib.sha256(section.encode()).hexdigest()
    document['copySha256'] = hashlib.sha256(copied.encode()).hexdigest()
    document['sectionLines'] = len(section.split('\n'))
    document['sectionBytes'] = len(section.encode())
    document['firstDifference'] = gate_section0.first_difference(section, copied)
    section_ok = section == copied

    # The amendment quote: markdown-wrapped, so words rather than bytes, and
    # the normalisation is printed so nobody has to trust it.
    source_words = unwrapped(fallback_source)
    copy_words = unwrapped('- ' + fallback_copy.lstrip('- '))
    document['amendmentSourceSha256'] = hashlib.sha256(
        source_words.encode()).hexdigest()
    document['amendmentCopySha256'] = hashlib.sha256(
        copy_words.encode()).hexdigest()
    document['amendmentSourceWords'] = source_words
    document['amendmentCopyWords'] = copy_words
    amendment_ok = source_words == copy_words
    document['AMENDMENT_VERBATIM'] = amendment_ok

    document['SECTION0_VERBATIM'] = bool(section_ok and amendment_ok)
    document['section'] = section
    if '--json' in sys.argv[1:] or not document['SECTION0_VERBATIM']:
        print(json.dumps(document, indent=1, ensure_ascii=False))
    else:
        print('SECTION0_VERBATIM: true '
              f'({document["sectionLines"]} lines, '
              f'{document["sectionBytes"]} bytes, '
              f'sha256 {document["sectionSha256"][:16]}…) '
              'AMENDMENT_VERBATIM: true')
    out = os.environ.get('ICS_OUT')
    if out:
        os.makedirs(out, exist_ok=True)
        with open(f'{out}/section0.json', 'w') as handle:
            json.dump(document, handle, indent=1, ensure_ascii=False)
    return 0 if document['SECTION0_VERBATIM'] else 1


if __name__ == '__main__':
    sys.exit(main())
