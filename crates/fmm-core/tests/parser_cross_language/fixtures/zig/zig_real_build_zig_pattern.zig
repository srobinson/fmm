const std = @import("std");
const builtin = @import("builtin");

pub const Package = struct {
    name: []const u8,
    version: []const u8,
    dependencies: []const Dependency,

    pub const Dependency = struct {
        name: []const u8,
        url: []const u8,
    };
};

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const lib = b.addStaticLibrary(.{
        .name = "mylib",
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });

    b.installArtifact(lib);

    const main_tests = b.addTest(.{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });

    const run_main_tests = b.addRunArtifact(main_tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_main_tests.step);
}

pub const version = "0.1.0";
pub const min_zig_version = "0.13.0";

comptime {
    const expected_zig = "0.13.0";
    _ = expected_zig;
}
