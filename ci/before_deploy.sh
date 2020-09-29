# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage= \
          bin=cookiebot

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    cargo rustc --bin $bin --target $TARGET --release -- -C lto

    cp target/$TARGET/release/$bin $stage/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $stage
}

main
