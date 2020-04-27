#!/bin/bash

DIR=`pwd`;

# Proc Macros
cd $DIR/socketio_proc;
cargo publish --allow-dirty;

# Thruster
cd $DIR/socketio_middleware;
cargo publish --allow-dirty;
