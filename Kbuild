obj-m := ax88179_178a.o
ax88179_178a-objs := ax88179_178a.rust.o

CARGO ?= cargo

export c_flags

$(src)/target/x86_64-linux-kernel/debug/libax88179_178a.a: cargo_will_determine_dependencies
	cd $(src); $(CARGO) doc -Z build-std=core,alloc --target=x86_64-linux-kernel; $(CARGO) build -Z build-std=core,alloc --target=x86_64-linux-kernel

.PHONY: cargo_will_determine_dependencies

%.rust.o: target/x86_64-linux-kernel/debug/lib%.a
	$(LD) -r -o $@ --whole-archive $<
