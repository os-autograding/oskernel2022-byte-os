function run(points, availablePoints, log) {
    let github = require("@actions/github")
    let request = require("request");
    console.log(request);
    console.log(github);
    log("github actor: ", github.actor)
}

module.exports.run = run;