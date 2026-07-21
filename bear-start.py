"""Start Bear.

    python bear-start.py

Options are for convenience; everything here is also reachable by right-clicking the board.
"""

from __future__ import annotations

import argparse
import sys


def main() -> int:
    parser = argparse.ArgumentParser(prog="bear", description="A chess board that plays like a person.")
    # Defaults are None so that stored settings win unless a flag is actually given.
    parser.add_argument("--model", help="maia3-5m, maia3-23m or maia3-79m")
    parser.add_argument("--elo", type=int, help="Maia's playing strength")
    parser.add_argument("--time", dest="time_control",
                        help="a time control, e.g. 5+0, 10+0, 15+10 or Unlimited")
    args = parser.parse_args()

    try:
        from PySide6.QtWidgets import QApplication
    except ImportError:
        print("Bear needs PySide6.\n\n    python -m pip install pyside6\n", file=sys.stderr)
        return 1

    from bear.window import BearWindow

    app = QApplication(sys.argv)
    app.setApplicationName("Bear")

    window = BearWindow(model=args.model, elo=args.elo, time_control=args.time_control)
    # Quitting by any route must still stop the engine thread.
    app.aboutToQuit.connect(window.shutdown)
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
