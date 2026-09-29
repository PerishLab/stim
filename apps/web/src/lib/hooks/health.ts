export type Health = { status: string };

export function probe(accept: (health: Health) => void): () => void {
	let live = true;
	fetch("/api/health")
		.then((response) => (response.ok ? response.json() : undefined))
		.then((body) => {
			if (live && body !== undefined) accept(body as Health);
		})
		.catch(() => undefined);
	return () => {
		live = false;
	};
}
