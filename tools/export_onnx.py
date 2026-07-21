"""Export the Maia 2 models to single-file ONNX for Bear to embed.

Run:  python tools/export_onnx.py [--out DIR]

Notes that cost time to rediscover:

* The `maia2` package pins `python <3.13` and `torch <2.9`. Both pins are conservative —
  the model builds and exports fine well past them — so this imports maia2 from a *clone*
  rather than installing it, which would drag a multi-gigabyte torch downgrade with it.
  Point MAIA2_REPO at your checkout.

* `model.eval()` is not optional. MAIA2Model carries dropout and the checkpoint loader does
  not switch it off, so exporting a training-mode graph bakes dropout into every move.

* torch's exporter writes weights to a sidecar `.onnx.data` by default. Bear ships one
  file, so the graph is re-saved with initialisers inlined.

* Elo inputs are *bucket indices* from `create_elo_dict()` (11 of them), not ratings.
  `map_to_category()` does the binning on the Python side; Bear must do it in C#.

* No opset is requested. Asking for 17 makes onnxscript attempt a downgrade that fails on
  this graph ("No initializer or constant input to node found"), prints a stack trace, and
  then leaves the model untouched anyway. The natural export is opset 18, which ONNX
  Runtime has supported for years.
"""

import argparse
import os
import sys
import time

MAIA2_REPO = os.environ.get("MAIA2_REPO", r"D:\GitHub\maia2")
sys.path.insert(0, MAIA2_REPO)

import torch  # noqa: E402
import onnx  # noqa: E402
import onnxruntime as ort  # noqa: E402
import numpy as np  # noqa: E402

from maia2.model import from_pretrained  # noqa: E402
from maia2.utils import create_elo_dict  # noqa: E402

MODELS = ("blitz", "rapid")


def export(kind: str, out_dir: str, cache: str) -> None:
    print(f"\n=== {kind} ===")
    model = from_pretrained(type=kind, device="cpu", save_root=cache)
    model.eval()  # dropout off, or it is baked into the graph

    params = sum(p.numel() for p in model.parameters())
    print(f"parameters: {params:,}  ({params * 4 / 1024 / 1024:.1f} MB fp32)")

    boards = torch.zeros(1, 18, 8, 8)
    elo_self = torch.tensor([len(create_elo_dict()) // 2])
    elo_oppo = torch.tensor([len(create_elo_dict()) // 2])

    with torch.no_grad():
        reference = model(boards, elo_self, elo_oppo)

    staged = os.path.join(out_dir, f"_staging_{kind}.onnx")
    final = os.path.join(out_dir, f"maia2-{kind}.onnx")

    t = time.time()
    torch.onnx.export(
        model,
        (boards, elo_self, elo_oppo),
        staged,
        input_names=["boards", "elo_self", "elo_oppo"],
        output_names=["policy", "side_info", "value"],
        dynamic_axes={
            "boards": {0: "batch"},
            "elo_self": {0: "batch"},
            "elo_oppo": {0: "batch"},
            "policy": {0: "batch"},
            "side_info": {0: "batch"},
            "value": {0: "batch"},
        },
    )
    print(f"exported in {time.time() - t:.1f}s")

    # Inline the sidecar weights: Bear embeds one resource, not a pair.
    onnx.save_model(onnx.load(staged), final, save_as_external_data=False)
    for leftover in (staged, staged + ".data"):
        if os.path.exists(leftover):
            os.remove(leftover)

    size = os.path.getsize(final) / 1024 / 1024
    print(f"wrote {final}  ({size:.1f} MB, single file)")

    # Acceptance test: the exported graph must agree with the PyTorch model it came from.
    session = ort.InferenceSession(final, providers=["CPUExecutionProvider"])
    got = session.run(None, {
        "boards": boards.numpy(),
        "elo_self": elo_self.numpy(),
        "elo_oppo": elo_oppo.numpy(),
    })

    worst = max(
        float(np.abs(a.detach().numpy() - b).max())
        for a, b in zip(reference, got)
    )
    print(f"max |torch - onnx|: {worst:.2e}  {'OK' if worst < 1e-4 else 'FAIL'}")

    runs = 20
    t = time.time()
    for _ in range(runs):
        session.run(None, {
            "boards": boards.numpy(),
            "elo_self": elo_self.numpy(),
            "elo_oppo": elo_oppo.numpy(),
        })
    print(f"cpu latency: {(time.time() - t) / runs * 1000:.1f} ms/move")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="artifacts/models")
    ap.add_argument("--cache", default="artifacts/maia2_checkpoints")
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    os.makedirs(args.cache, exist_ok=True)

    for kind in MODELS:
        export(kind, args.out, args.cache)


if __name__ == "__main__":
    main()
