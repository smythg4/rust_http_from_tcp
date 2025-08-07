# HTTP from TCP

This is a boot.dev course written for implementation in Go that I've ported over into Rust as close as possible.

## Thoughts
- I wasn't able to deal with generic handlers. Go used function pointers, but I kept running into lifetime issues when I tried it in Rust.
- I'd be interested to explore a router to make the handler a little less burdensome.
