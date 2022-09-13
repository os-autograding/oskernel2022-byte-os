function run({points, availablePoints}, { log, github, request }) {
    console.log(github);
    console.log(github.sender);
    console.log(github.pusher);
    console.log(github.repository)
    log(request.post);
}

module.exports.run = run;