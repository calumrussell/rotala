Snake is a PoC running a Rust backtest from Python. Eventual goal is:

* Create zero-copy data structures in alator from Python.
* Create a trading strategy in Python that can be run in alator.

Maturin setup:

Develop:
```
    python3 -m venv venv
    maturin develop
```
Build:
```
    maturin build
```
