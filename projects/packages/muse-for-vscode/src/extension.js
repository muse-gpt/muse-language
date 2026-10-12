const vscode = require("vscode");

function activate(context) {
    const disposable = vscode.commands.registerCommand("muse.openGuide", () => {
        vscode.env.openExternal(
            vscode.Uri.parse("https://github.com/muse-gpt/muse-language#readme"),
        );
    });
    context.subscriptions.push(disposable);
}

module.exports = { activate };
