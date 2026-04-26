rustc --edition 2021 scratch.rs --extern render_slides=target/debug/librender_slides.so -L dependency=target/debug/deps
LD_LIBRARY_PATH=target/debug ./scratch
