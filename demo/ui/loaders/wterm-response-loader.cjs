module.exports = function (source) {
    // Protocol replies must not masquerade as user input and flush the editor.
    const replacements = [
        ["        this.onData = options.onData || null;",
            "        this.onData = options.onData || null;\n        this.onResponse = options.onResponse || null;"],
        ["                if (this.onData)\n                    this.onData(response);",
            "                this.onResponse?.(response);"],
        ["                this.onData?.(this._windowSizeResponse(query));",
            "                this.onResponse?.(this._windowSizeResponse(query));"],
    ];
    for (const [original, replacement] of replacements) {
        if (source.split(original).length !== 2) {
            throw new Error("Wterm response delivery changed; review the protocol reply integration");
        }
        source = source.replace(original, replacement);
    }
    return source;
};
