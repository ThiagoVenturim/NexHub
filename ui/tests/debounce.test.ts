import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { debounce } from "../src/canvas/layout";

describe("debounce utility", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delays execution and coalesces multiple rapid calls", () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 500);

    debounced("call 1");
    debounced("call 2");
    debounced("call 3");

    expect(fn).not.toHaveBeenCalled();

    // Advance 499ms: still not called
    vi.advanceTimersByTime(499);
    expect(fn).not.toHaveBeenCalled();

    // Advance 1ms more (500ms): called exactly once with latest argument
    vi.advanceTimersByTime(1);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith("call 3");
  });

  it("flush executes immediately without waiting", () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 800);

    debounced("data to save");
    expect(fn).not.toHaveBeenCalled();

    debounced.flush();
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith("data to save");

    // Later timer tick does not re-invoke
    vi.advanceTimersByTime(1000);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("cancel prevents pending execution", () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 500);

    debounced("will be cancelled");
    expect(fn).not.toHaveBeenCalled();

    debounced.cancel();

    vi.advanceTimersByTime(1000);
    expect(fn).not.toHaveBeenCalled();
  });
});
