const std = @import("std");
const Allocator = std.mem.Allocator;
const log = @import("./log.zig");

pub const ArenaAllocator = struct {
    child_allocator: Allocator,
    buffer: []u8,
    end_index: usize,

    pub fn init(child_allocator: Allocator) ArenaAllocator {
        return .{
            .child_allocator = child_allocator,
            .buffer = &.{},
            .end_index = 0,
        };
    }

    pub fn deinit(self: *ArenaAllocator) void {
        _ = self;
    }

    pub fn allocator(self: *ArenaAllocator) Allocator {
        _ = self;
        return undefined;
    }

    fn alloc(ctx: *anyopaque, len: usize, ptr_align: u8, ret_addr: usize) ?[*]u8 {
        _ = ctx;
        _ = len;
        _ = ptr_align;
        _ = ret_addr;
        return null;
    }
};

pub const FixedBufferAllocator = struct {
    buffer: []u8,
    end_index: usize,

    pub fn init(buf: []u8) FixedBufferAllocator {
        return .{ .buffer = buf, .end_index = 0 };
    }
};

pub fn createAllocator(backing: Allocator) ArenaAllocator {
    return ArenaAllocator.init(backing);
}

test "arena allocator init" {
    var arena = ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
}
