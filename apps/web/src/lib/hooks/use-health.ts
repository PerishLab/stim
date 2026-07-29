import { useEffect, useState } from "react";

export type Health = { status: string };

export function useHealth(): Health | undefined {
	const [health, setHealth] = useState<Health | undefined>(undefined);
	useEffect(() => {
		let live = true;
		fetch("/api/health")
			.then((response) => (response.ok ? response.json() : undefined))
			.then((body) => {
				if (live) setHealth(body as Health | undefined);
			})
			.catch(() => undefined);
		return () => {
			live = false;
		};
	}, []);
	return health;
}
