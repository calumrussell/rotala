[Docs](https://docs.rs/alator)

# What is Alator?

Alator is the front-end for Rotala, a backtesting library built with Rust.

Rotala contains all the back-end exchange code. Alator demonstrates how this back-end code can be used to run backtests. Rotala is built to run as a JSON service but exchanges can be imported as a library, Alator uses the latter feature.

Alator started out as a backtesting library used in another application. Over time it became clear that backtesting was a standalone application, and then it moved towards developing something cross-language that could run on a server. This transition is still at an early stage and this isn't a production application.
