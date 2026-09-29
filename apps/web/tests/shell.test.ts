import Home from "@stim/web/views/index.svelte";
import { render } from "svelte/server";
import { expect, test } from "vitest";

test("home renders the pane before the api answers", () => {
	const markup = render(Home).body;
	expect(markup).toContain("stim");
	expect(markup).toContain('class="frame');
	expect(markup).toContain("The api has not answered yet.");
});
