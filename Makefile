VERSION=$(shell grep ^version Cargo.toml|cut -d\" -f2)

all: test

tag:
	git tag -a v${VERSION} -m v${VERSION}
	git push origin --tags

release: cpub tag pkg

cpub:
	cargo publish

pkg:
	rm -rf _build
	mkdir -p _build
	cross build --target x86_64-unknown-linux-musl --release
	cross build --target aarch64-unknown-linux-musl --release
	cd target/x86_64-unknown-linux-musl/release && cp gmg /opt/gmg/_build/gmg-${VERSION}-x86_64
	cd target/aarch64-unknown-linux-musl/release && \
		aarch64-linux-gnu-strip gmg && \
		cp gmg /opt/gmg/_build/gmg-${VERSION}-aarch64
	cd _build && echo "" | gh release create v$(VERSION) -t "v$(VERSION)" \
		gmg-${VERSION}-x86_64 \
		gmg-${VERSION}-aarch64
