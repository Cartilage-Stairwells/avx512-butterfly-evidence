#!/usr/bin/env python3
"""Check that the sealed SMT models still match the source they claim to model.

Three outcomes, never merged into one line:
  SCRIPT_INTEGRITY  the .smt2 files are the ones bound in the record
  SOURCE_BINDING    the source files are the ones bound in the record
  RESULT            PASS (both), FAIL (a mismatch), INCOMPLETE (source unavailable)

Exit 0 PASS, 1 FAIL, 2 INCOMPLETE. A missing source is INCOMPLETE, never PASS,
because an absent comparison is not a successful comparison.
"""
import argparse, hashlib, json, os, sys

def sha(p):
    h=hashlib.sha256()
    with open(p,'rb') as f:
        for c in iter(lambda: f.read(65536), b''):
            h.update(c)
    return h.hexdigest()

ap=argparse.ArgumentParser()
ap.add_argument("--bindings", default=os.path.join(os.path.dirname(__file__),"SMT_MODEL_BINDINGS.json"))
ap.add_argument("--source-root", default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
a=ap.parse_args()

rec=json.load(open(a.bindings))
scrdir=os.path.dirname(os.path.abspath(a.bindings))

bad_s, bad_b = [], []
miss_s, miss_b = [], []
for name, meta in sorted(rec["scripts"].items()):
    p=os.path.join(scrdir,name)
    if not os.path.isfile(p): miss_s.append(name); continue
    if sha(p)!=meta["sha256"]: bad_s.append(name)

for rel, meta in sorted(rec["source"].items()):
    p=os.path.join(a.source_root, rel)
    if not meta.get("sha256"):
        bad_b.append(f"{rel} (unbound in record)"); continue
    if not os.path.isfile(p):
        miss_b.append(rel); continue
    if sha(p)!=meta["sha256"]: bad_b.append(rel)

n=len(rec["scripts"])
if bad_s or miss_s:
    print(f"SCRIPT_INTEGRITY: FAIL ({len(bad_s)} mismatched, {len(miss_s)} missing of {n})")
    for x in bad_s[:5]: print(f"  mismatch: {x}")
    for x in miss_s[:5]: print(f"  missing:  {x}")
else:
    print(f"SCRIPT_INTEGRITY: PASS ({n} of {n} match the binding record)")

src_total=len(rec["source"]); src_seen=sum(1 for r in rec["source"] if os.path.isfile(os.path.join(a.source_root,r)))
if bad_b:
    print(f"SOURCE_BINDING: FAIL ({len(bad_b)} of {src_total} source files changed since binding)")
    for x in bad_b: print(f"  drifted: {x}")
elif src_seen<src_total:
    print(f"SOURCE_BINDING: INCOMPLETE ({src_seen} of {src_total} source files present)")
    for x in miss_b: print(f"  absent: {x}")
else:
    print(f"SOURCE_BINDING: PASS ({src_seen} of {src_total} source files match)")

if bad_s or bad_b:
    print("RESULT: FAIL"); print("called_at:", rec["bound_at_utc"]); sys.exit(1)
if miss_s or src_seen<src_total:
    print("RESULT: INCOMPLETE"); print("called_at:", rec["bound_at_utc"]); sys.exit(2)
print("RESULT: PASS"); print("called_at:", rec["bound_at_utc"]); sys.exit(0)
