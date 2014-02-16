const vscode = require("vscode");

function activate(context) {
    const disposable = vscode.commands.registerCommand("muse.openGuide", () => {
        vscode.env.openExternal(
            vscode.Uri.parse("http://localhost:5173/language"),
        );
    });
    context.subscriptions.push(disposable);
}

module.exports = { activate };
