VERSION=0.1.0

all: test

tag:
	git tag -a v${VERSION} -m v${VERSION}
	git push origin --tags

ver:
	sed -i 's/^version = ".*/version = "${VERSION}"/g' Cargo.toml

release: cpub tag pkg

cpub:
	cargo publish

pkg:
	rm -rf _build
	mkdir -p _build
	cross build --target x86_64-unknown-linux-musl --release
	cross build --target i686-unknown-linux-musl --release
	cross build --target arm-unknown-linux-musleabihf --release
	cross build --target aarch64-unknown-linux-musl --release
	cd target/x86_64-unknown-linux-musl/release && cp gmg ../../../_build/gmg-${VERSION}-x86_64
	cd target/i686-unknown-linux-musl/release && cp gmg ../../../_build/gmg-${VERSION}-i686
	cd target/arm-unknown-linux-musleabihf/release && cp gmg ../../../_build/gmg-${VERSION}-arm-musleabihf
	cd target/aarch64-unknown-linux-musl/release && \
		aarch64-linux-gnu-strip gmg && \
		cp gmg ../../../_build/gmg-${VERSION}-aarch64
	cd _build && echo "" | gh release create v$(VERSION) -t "v$(VERSION)" \
		gmg-${VERSION}-arm-musleabihf \
		gmg-${VERSION}-i686 \
		gmg-${VERSION}-x86_64 \
		gmg-${VERSION}-aarch64
