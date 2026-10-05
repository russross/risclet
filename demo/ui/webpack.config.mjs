import { fileURLToPath } from "node:url";

export default {
    mode: "production",
    entry: "./index.ts",
    output: {
        path: fileURLToPath(new URL("../build/ui", import.meta.url)),
        filename: "app-[contenthash].js",
        clean: true,
    },
    module: { rules: [
        { test: /\.ts$/, use: "ts-loader", exclude: /node_modules/ },
        { test: /\.css$/i, use: ["style-loader", "css-loader"] },
    ] },
    resolve: {
        extensions: [".ts", ".js"],
        extensionAlias: { ".js": [".ts", ".js"] },
    },
};
