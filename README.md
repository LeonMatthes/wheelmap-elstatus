# ElStatus

Query elevator status from the [WheelMap](https://wheelmap.org/) API.

This project uses Rust - simply use `cargo run` to get a list of available commands.

To use the `display` functionality, you will need to set up an e-paper display with [OpenEPaperLink](https://github.com/OpenEPaperLink/OpenEPaperLink).
Currently only 296x128 red-white-black displays are supported.

## Wheelmap API access

ElStatus requires a WheelMap API access token.
To provide your own, set the WHEELMAP_TOKEN environment variable to the corresponding value.
