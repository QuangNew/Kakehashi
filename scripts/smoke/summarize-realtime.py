"""Summarize benchmark JSON without printing podcast transcripts or captions."""
import argparse
import json
import math
from pathlib import Path
import statistics
import unicodedata


def stats(values):
    values = sorted(v for v in values if v is not None)
    if not values:
        return None
    return {"count": len(values), "median": round(statistics.median(values), 1),
            "p95": values[math.ceil(len(values) * .95) - 1], "max": values[-1]}


def normalize(text):
    return ''.join(ch for ch in unicodedata.normalize('NFKC', text).lower()
                   if unicodedata.category(ch)[0] in ('L', 'N'))


def distance(reference, actual):
    row = list(range(len(actual) + 1))
    for i, left in enumerate(reference, 1):
        following = [i]
        for j, right in enumerate(actual, 1):
            following.append(min(following[-1] + 1, row[j] + 1, row[j - 1] + (left != right)))
        row = following
    return row[-1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    parser.add_argument('--captions', type=Path)
    parser.add_argument('--clip-start', type=float, default=30)
    parser.add_argument('--seconds', type=float, default=90)
    args = parser.parse_args()
    reference = None
    if args.captions:
        captions = json.loads(args.captions.read_text(encoding='utf-8-sig'))
        start, end = args.clip_start * 1000, (args.clip_start + args.seconds) * 1000
        reference = normalize(''.join(''.join(part.get('utf8', '') for part in event.get('segs', []))
                                     for event in captions['events']
                                     if event.get('tStartMs', 0) < end and
                                     event.get('tStartMs', 0) + event.get('dDurationMs', 0) > start))
    for path in args.logs:
        records = []
        for line in path.read_text(encoding='utf-8-sig', errors='replace').splitlines():
            if line.startswith('{'):
                try:
                    records.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
        finals = [r for r in records if r.get('kind') == 'transcript' and r.get('final')]
        translations = [r for r in records if r.get('kind') == 'translation']
        raw = [r for r in records if r.get('kind') == 'raw']
        final_perf = next((r['stt'] for r in reversed(records) if r.get('kind') == 'performance_final'), None)
        summary = {
            'log': str(path), 'raw_inference_ms': stats([r['inference_ms'] for r in raw]),
            'finals': len(finals), 'translations': len(translations),
            'stt_after_audio_ms': stats([r.get('lag_ms') for r in finals]),
            'translation_after_audio_ms': stats([r.get('lag_ms') for r in translations]),
            'translation_inference_ms': stats([r['inference_ms'] for r in translations]),
            'translation_queue_ms': stats([r['queue_ms'] for r in translations]),
            'performance': final_perf,
            'peak_stt_queued_audio_ms': max((r['stt']['queued_audio_ms'] for r in records
                                           if r.get('kind') == 'performance' and r.get('stt')), default=0),
            'translation_errors': sum(r.get('kind') == 'translation_error' for r in records),
        }
        if final_perf and final_perf.get('processed_audio_ms'):
            summary['stt_rtf'] = round(final_perf['total_inference_ms'] / final_perf['processed_audio_ms'], 4)
        if reference:
            actual = normalize(''.join(r.get('text') or '' for r in (raw or finals)))
            if actual:
                edits = distance(reference, actual)
                summary['caption_comparison'] = {'reference_chars': len(reference), 'recognized_chars': len(actual),
                                                 'edits': edits, 'cer': round(edits / len(reference), 4)}
        print(json.dumps(summary, ensure_ascii=True))


if __name__ == '__main__':
    main()
