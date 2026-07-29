import { Frame, Hero, Note } from "@perish/react-components";
import type { ReactNode } from "react";
import { useHealth } from "../lib/hooks/health";

export default function Home(): ReactNode {
	const health = useHealth();
	return (
		<Frame>
			<Hero
				title="stim"
				text="a message product downstream of santi"
				mark="◆"
			/>
			{health === undefined ? (
				<Note text="The api has not answered yet." tone="warn" />
			) : (
				<Note text={`api: ${health.status}`} />
			)}
		</Frame>
	);
}
