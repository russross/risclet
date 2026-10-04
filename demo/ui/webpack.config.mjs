import { fileURLToPath } from "node:url";
import { terminalRules } from "./loaders/webpack.mjs";

export default {
    mode: "production",
    entry: "./index.ts",
    output: {
        path: fileURLToPath(new URL("../build/ui", import.meta.url)),
        filename: "app.js",
        chunkFilename: "chunk-[contenthash].js",
        assetModuleFilename: "[name]-[contenthash][ext]",
        clean: true,
    },
    module: { rules: [
        ...terminalRules(),
        { test: /\.ts$/, use: "ts-loader", exclude: /node_modules/ },
        { test: /\.css$/i, use: ["style-loader", "css-loader"] },
    ] },
    resolve: {
        extensions: [".ts", ".js"],
        extensionAlias: { ".js": [".ts", ".js"] },
    },
};
