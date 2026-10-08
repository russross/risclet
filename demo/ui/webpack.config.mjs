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
        { test: /\.ts$/, use: { loader: "ts-loader", options: {
            configFile: fileURLToPath(new URL("./tsconfig.json", import.meta.url)),
        } }, exclude: /node_modules/ },
        { test: /\.css$/i, use: ["style-loader", "css-loader"] },
    ] },
    resolve: {
        modules: [fileURLToPath(new URL("./node_modules", import.meta.url)), "node_modules"],
        extensions: [".ts", ".js"],
        extensionAlias: { ".js": [".ts", ".js"] },
    },
};
