// --- FMM ---
// exports: [Config, Engine, Pipeline, Point, Status, process]
// imports: [algorithm, memory, string, vector]
// dependencies: [config.h, utils/helpers.h]
// namespaces: [engine, utils]
// ---

#include <vector>
#include <string>
#include <memory>
#include <algorithm>
#include "config.h"
#include "utils/helpers.h"

namespace engine {

struct Point {
    double x, y, z;
};

enum Status {
    Active,
    Inactive,
    Pending
};

class Config {
public:
    std::string host;
    int port;
    bool debug;

    Config(const std::string& h, int p, bool d)
        : host(h), port(p), debug(d) {}
};

class Engine {
public:
    void start();
    void stop();
    bool isRunning() const;

private:
    bool running_ = false;
    Config config_;
};

template <typename T>
class Pipeline {
public:
    void add(T item) {
        items_.push_back(std::move(item));
    }

    size_t size() const {
        return items_.size();
    }

private:
    std::vector<T> items_;
};

} // namespace engine

namespace utils {

void process(const std::vector<std::string>& data) {
    std::vector<std::string> result;
    std::copy_if(data.begin(), data.end(), std::back_inserter(result),
        [](const std::string& s) { return !s.empty(); });
}

} // namespace utils
