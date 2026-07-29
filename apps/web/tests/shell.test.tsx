import { expect, test } from "vitest";
import Home from "../src/views/index";

test("home is a component", () => {
	expect(typeof Home).toBe("function");
});
