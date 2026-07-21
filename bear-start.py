"""Start Bear.

    python bear-start.py

Options are for convenience; everything here is also reachable by right-clicking the board.
"""

from __future__ import annotations

import argparse
import sys


def main() -> int:
    parser = argparse.ArgumentParser(prog="bear", description="A chess board that plays like a person.")
    parser.add_argument("--model", default="maia3-5m",
                        help="maia3-5m, maia3-23m or maia3-79m")
    parser.add_argument("--elo", type=int, default=1500, help="Maia's playing strength")
    parser.add_argument("--minutes", type=float, default=10, help="clock, per side")
    parser.add_argument("--increment", type=float, default=0, help="increment, seconds")
    args = parser.parse_args()

    try:
        from PySide6.QtWidgets import QApplication
    except ImportError:
        print("Bear needs PySide6.\n\n    python -m pip install pyside6\n", file=sys.stderr)
        return 1

    from bear.window import BearWindow

    app = QApplication(sys.argv)
    app.setApplicationName("Bear")

    window = BearWindow(model=args.model, elo=args.elo,
                        minutes=args.minutes, increment=args.increment)
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
