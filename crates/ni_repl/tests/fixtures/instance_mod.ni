// Module that exports a pre-built instance

class Config:
    fun init(name, value):
        self.name = name
        self.value = value

    fun describe():
        return self.name + "=" + to_string(self.value)

const DEFAULT = Config("mode", 42)
