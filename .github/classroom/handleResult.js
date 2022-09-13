function run({points, availablePoints}, { log, github, request }) {
    log(github.actor);
    log(request.post);
}

module.exports.run = run;