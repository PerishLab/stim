import { cli, flags } from "@perish/sealkit/cli";
import { init } from "@perish/sealkit/init";
import { io } from "@perish/sealkit/io";

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
flags(args).positionals("init", { allowHelp: true });
if (flags(args).help()) {
  io.print("Usage: runseal :init");
  io.print("");
  io.print("Validate the repository and install versioned git hooks.");
  Deno.exit(0);
}

await init({
  tools: [
    "git",
    "deno",
    "cargo",
    "tea",
    "runseal",
    "sh",
  ],
  paths: [
    "Cargo.toml",
    "Cargo.lock",
    "negentropy.toml",
    "vocabulary.toml",
    "manage.sh",
    "runseal.toml",
    ".runseal/deno.json",
    ".runseal/deno.lock",
    ".runseal/wrappers/guard.ts",
    ".runseal/wrappers/init.ts",
    ".runseal/wrappers/land.ts",
    ".forgejo/workflows/guard.yml",
  ],
});
