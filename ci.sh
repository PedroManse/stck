#! /usr/bin/env bash
set -e

if [ -n "$FIX" ] && [ "$FIX" != "0" ] ; then
	fix="--fix"
fi

if [ -n "$DIRTY" ] && [ "$DIRTY" != "0" ] ; then
	allow_dirty="--allow-dirty"
fi


ci() {
	pushd $1
	set -x

	cargo fmt
	cargo build
	cargo doc
	cargo test
	cargo clippy $fix $allow_dirty --all-targets --all-features -- \
		-Dclippy::perf \
		-Dclippy::style \
		-Wclippy::pedantic \
		-Aclippy::unnested_or_patterns \
		-Aclippy::wildcard_imports \
		-Aclippy::enum_glob_use \
		-Aclippy::too_many_lines \
		-Aclippy::match_same_arms \
		-Aclippy::unnecessary_wraps \
		-Aclippy::missing_errors_doc \
		-Aclippy::cast_sign_loss \
		-Aclippy::cast_possible_wrap \
		-Aclippy::cast_possible_truncation

	set +x
	popd
}

if [ "$#" != 0 ] ; then
	for target in "$@" ; do
		ci $target
	done
else
	ci .
	ci usage
fi

